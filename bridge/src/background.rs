//! The entry point a platform background task calls.
//!
//! Not part of the flutter_rust_bridge API, and deliberately so. A
//! `BGTaskScheduler` handler on iOS — and a `WorkManager` worker on Android —
//! runs with no Flutter engine: there is no Dart VM, no isolate, nothing to
//! carry a generated binding. Cargokit already links this archive statically
//! into the app binary, so the platform code can call straight into Rust
//! through a plain C symbol and skip the engine entirely.
//!
//! The alternative is spinning up a headless Flutter engine inside the
//! handler. That costs most of the budget iOS grants — on the order of thirty
//! seconds, and less on a device it judges busy — before any link is checked.
//!
//! Everything here is written for a caller that cannot handle a panic. Rust
//! unwinding across an FFI boundary is undefined behaviour, so each entry
//! catches its own and reports failure as a return code instead.

use std::ffi::{c_char, c_int, CStr};
use std::path::PathBuf;
use std::sync::Arc;

use websave_core::{Monitor, MonitorConfig, Vault};

/// A sweep ran and this many links were checked. Zero is a success: it
/// usually means nothing was due.
pub const WEBSAVE_BG_OK: c_int = 0;
/// The path argument was null, or not valid UTF-8.
pub const WEBSAVE_BG_BAD_PATH: c_int = -1;
/// The vault would not open.
pub const WEBSAVE_BG_VAULT_FAILED: c_int = -2;
/// Something panicked. The task should be treated as failed and rescheduled.
pub const WEBSAVE_BG_PANICKED: c_int = -3;

/// Check every link whose scheduled time has arrived.
///
/// `vault_path` is a NUL-terminated UTF-8 path to the vault *directory*,
/// passed in rather than derived here because only the platform layer knows
/// its own container. The buffer is borrowed for the duration of the call and
/// never retained.
///
/// Returns the number of links checked, or one of the negative codes above.
/// A caller should treat any negative value as a failed task.
///
/// # Safety
///
/// `vault_path` must be either null or a valid NUL-terminated C string that
/// stays alive for the duration of the call.
///
/// This opens its own [`Vault`] rather than sharing the one the Dart bridge
/// holds. The background process may not be the foreground app at all, and on
/// iOS usually is not; SQLite in WAL mode is what makes the two safe
/// side by side, which this app already depends on for the menubar companion.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn websave_run_due_checks(vault_path: *const c_char) -> c_int {
    // Catch before anything else: a panic crossing this boundary is UB, and
    // the caller is a system daemon that will simply kill the app.
    std::panic::catch_unwind(|| {
        if vault_path.is_null() {
            return WEBSAVE_BG_BAD_PATH;
        }
        let path = match unsafe { CStr::from_ptr(vault_path) }.to_str() {
            Ok(path) => PathBuf::from(path),
            Err(_) => return WEBSAVE_BG_BAD_PATH,
        };

        let vault = match Vault::open(path) {
            Ok(vault) => Arc::new(vault),
            // Nowhere to log this: the log lives in the vault that would not
            // open. The return code is all the caller gets.
            Err(_) => return WEBSAVE_BG_VAULT_FAILED,
        };

        // Defaults, with one change: a background sweep is capped so it
        // cannot run past the window the OS granted. Being killed mid-sweep
        // is not a correctness problem — each link is committed as it settles
        // and the rest stay due — but iOS counts a task it had to kill
        // against future scheduling, so it is worth finishing early instead.
        let config = MonitorConfig {
            max_per_sweep: Some(BACKGROUND_SWEEP_CAP),
            ..MonitorConfig::default()
        };

        let summary = Monitor::new(vault.clone(), config).run_due();

        // The whole reason the activity log exists. This runs with no engine,
        // on a schedule the OS picks, usually while the phone is in a pocket —
        // so unless it writes down that it ran, there is no way to answer "are
        // my links still being checked" from inside the app.
        vault.log_activity(
            "info",
            "background",
            &format!(
                "Checked {} link(s): {} unchanged, {} changed, {} dead, {} unreachable",
                summary.checked,
                summary.not_modified,
                summary.changed,
                summary.dead,
                summary.unreachable
            ),
        );

        c_int::try_from(summary.checked).unwrap_or(c_int::MAX)
    })
    .unwrap_or(WEBSAVE_BG_PANICKED)
}

/// How many links one background sweep will check before stopping.
///
/// iOS grants roughly thirty seconds and is stricter on a busy or cold
/// device. Most checks settle with a 304 in well under a second, so this
/// leaves headroom for a handful of slow hosts while still finishing well
/// inside the budget. Anything left over stays due for the next run.
const BACKGROUND_SWEEP_CAP: usize = 40;

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::CString;

    /// A null path must be reported, not dereferenced.
    #[test]
    fn null_path_is_rejected() {
        assert_eq!(
            unsafe { websave_run_due_checks(std::ptr::null()) },
            WEBSAVE_BG_BAD_PATH
        );
    }

    /// An unopenable vault returns a code rather than unwinding into the
    /// caller, which is the whole point of the catch_unwind above.
    #[test]
    fn unopenable_vault_is_reported() {
        // A directory inside a regular file cannot exist on any OS. The
        // obvious `/dev/null/...` is only unopenable on Unix; on Windows it
        // reads as `C:\dev\null\...`, which opens fine and leaves a stray
        // vault on the drive.
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("a-file");
        std::fs::write(&file, b"").unwrap();
        let vault = file.join("vault");
        let path = CString::new(vault.to_str().unwrap()).unwrap();
        let code = unsafe { websave_run_due_checks(path.as_ptr()) };
        assert_eq!(code, WEBSAVE_BG_VAULT_FAILED);
    }

    /// An empty but valid vault is a success with nothing checked — the
    /// ordinary outcome when nothing is due.
    #[test]
    fn empty_vault_checks_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let path = CString::new(dir.path().to_str().unwrap()).unwrap();
        let code = unsafe { websave_run_due_checks(path.as_ptr()) };
        assert_eq!(code, WEBSAVE_BG_OK);
    }
}

// ---- Android ----
//
// Kotlin cannot call the C symbol above directly: the JVM resolves a native
// method by a mangled name built from the package, class and method, so the
// export has to be shaped to match. Cargokit builds this crate as a cdylib on
// Android, so the .so is already inside the APK and System.loadLibrary finds
// it without any extra packaging step.

/// JNI entry for `LinkCheckWorker.nativeRunDueChecks`.
///
/// The name is not free-form: it must stay in lockstep with the Kotlin
/// package, class and method, or the JVM raises UnsatisfiedLinkError at the
/// first call rather than at load. Renaming or moving that class means
/// renaming this.
///
/// Mirrors [`websave_run_due_checks`] — same return codes, same panic
/// containment — differing only in unwrapping the Java string first.
///
/// # Safety
///
/// Called by the JVM with a valid environment and a valid string.
#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_venugopal_websave_1app_LinkCheckWorker_nativeRunDueChecks(
    mut env: jni::JNIEnv,
    _class: jni::objects::JClass,
    vault_path: jni::objects::JString,
) -> c_int {
    // A panic unwinding into the JVM aborts the process, so this is caught
    // here for the same reason as the C entry point.
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let Ok(path) = env.get_string(&vault_path) else {
            return WEBSAVE_BG_BAD_PATH;
        };
        let path: String = path.into();

        let vault = match Vault::open(PathBuf::from(path)) {
            Ok(vault) => Arc::new(vault),
            Err(_) => return WEBSAVE_BG_VAULT_FAILED,
        };

        let config = MonitorConfig {
            // Android's WorkManager is far more generous than iOS — ten
            // minutes rather than thirty seconds — but a doze-mode wakeup is
            // still not the moment to sweep a whole vault, and leftovers stay
            // due for the next run either way.
            max_per_sweep: Some(BACKGROUND_SWEEP_CAP),
            ..MonitorConfig::default()
        };

        let summary = Monitor::new(vault.clone(), config).run_due();
        vault.log_activity(
            "info",
            "background",
            &format!(
                "Checked {} link(s): {} unchanged, {} changed, {} dead, {} unreachable",
                summary.checked,
                summary.not_modified,
                summary.changed,
                summary.dead,
                summary.unreachable
            ),
        );
        c_int::try_from(summary.checked).unwrap_or(c_int::MAX)
    }))
    .unwrap_or(WEBSAVE_BG_PANICKED)
}
