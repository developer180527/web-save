//! System tray (macOS menubar / Windows notification area) quick access.
//!
//! The dropdown menu is a real native menu (NSMenu on macOS) built
//! dynamically from the vault: starred saves, recent saves, then app
//! controls. It is rebuilt whenever saves change.
//!
//! Platform behavior:
//! - Left click toggles the quick-access popover panel (rich list view with
//!   favicons and search; on macOS it renders as an arrowed NSPopover-style
//!   card).
//! - Right click opens the native dropdown menu (starred/recent/controls) —
//!   also the fallback entry point on Linux, where tray click events are
//!   often not delivered.
//!
//! On macOS none of this runs: the native SwiftUI menubar app
//! (macos-menubar/) owns quick access there, so most of this module is
//! intentionally unused on that platform.
#![cfg_attr(target_os = "macos", allow(dead_code))]

use std::sync::Mutex;
use std::time::{Duration, Instant};

use tauri::menu::{IsMenuItem, Menu, MenuBuilder, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager, PhysicalPosition, WebviewWindow, Wry};
use tauri_plugin_opener::OpenerExt;
use websave_core::{ListQuery, Save, Vault};

pub const QUICK_WINDOW: &str = "quick";
const TRAY_ID: &str = "main-tray";
const EDGE_MARGIN: f64 = 8.0;
const MAX_STARRED: i64 = 12;
const MAX_RECENT: usize = 5;
const TITLE_MAX_CHARS: usize = 48;

/// Hide-on-blur and click-to-toggle race (popover platforms only): clicking
/// the tray while the panel is open first blurs (hiding it), then delivers
/// the click (which would re-show it). Remember when blur last hid the panel
/// so that click can be treated as "close" instead.
pub struct QuickPanelGuard(Mutex<Instant>);

impl QuickPanelGuard {
    pub fn new() -> Self {
        QuickPanelGuard(Mutex::new(Instant::now() - Duration::from_secs(60)))
    }

    pub fn mark_hidden(&self) {
        *self.0.lock().unwrap() = Instant::now();
    }

    fn hidden_just_now(&self) -> bool {
        self.0.lock().unwrap().elapsed() < Duration::from_millis(300)
    }
}

pub fn setup(app: &AppHandle, vault: &Vault) -> tauri::Result<()> {
    let menu = build_menu(app, vault)?;
    let mut tray = TrayIconBuilder::with_id(TRAY_ID)
        .tooltip("WebSave")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id().as_ref() {
            "open" => show_main(app),
            "quit" => app.exit(0),
            id => {
                if let Some(url) = id.strip_prefix("url:") {
                    let _ = app.opener().open_url(url, None::<&str>);
                }
            }
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                position,
                ..
            } = event
            {
                toggle_quick_panel(tray.app_handle(), position);
            }
        });
    if let Some(icon) = app.default_window_icon() {
        tray = tray.icon(icon.clone());
    }
    tray.build(app)?;
    Ok(())
}

/// Rebuild the tray dropdown from current vault contents.
/// Must run on the main thread (menus are AppKit objects on macOS).
pub fn rebuild_menu(app: &AppHandle, vault: &Vault) {
    let Some(tray) = app.tray_by_id(TRAY_ID) else {
        return;
    };
    match build_menu(app, vault) {
        Ok(menu) => {
            let _ = tray.set_menu(Some(menu));
        }
        Err(e) => log::warn!("tray: failed to rebuild menu: {e}"),
    }
}

fn build_menu(app: &AppHandle, vault: &Vault) -> tauri::Result<Menu<Wry>> {
    let starred = vault
        .list_saves(&ListQuery {
            favorites_only: true,
            limit: Some(MAX_STARRED),
            ..Default::default()
        })
        .unwrap_or_default();
    let recent: Vec<Save> = vault
        .list_saves(&ListQuery {
            limit: Some(MAX_STARRED + MAX_RECENT as i64),
            ..Default::default()
        })
        .unwrap_or_default()
        .into_iter()
        .filter(|s| !s.favorite)
        .take(MAX_RECENT)
        .collect();

    let mut items: Vec<Box<dyn IsMenuItem<Wry>>> = Vec::new();
    items.push(header(app, "hdr-starred", "Starred")?);
    if starred.is_empty() {
        items.push(header(app, "hdr-no-starred", "No starred saves yet")?);
    }
    for save in &starred {
        items.push(save_item(app, save)?);
    }
    if !recent.is_empty() {
        items.push(Box::new(PredefinedMenuItem::separator(app)?));
        items.push(header(app, "hdr-recent", "Recent")?);
        for save in &recent {
            items.push(save_item(app, save)?);
        }
    }
    items.push(Box::new(PredefinedMenuItem::separator(app)?));
    items.push(Box::new(MenuItem::with_id(
        app,
        "open",
        "Open WebSave",
        true,
        None::<&str>,
    )?));
    items.push(Box::new(MenuItem::with_id(
        app,
        "quit",
        "Quit WebSave",
        true,
        None::<&str>,
    )?));

    let refs: Vec<&dyn IsMenuItem<Wry>> = items.iter().map(|i| i.as_ref()).collect();
    MenuBuilder::new(app).items(&refs).build()
}

/// Disabled items render as grey section headers.
fn header(app: &AppHandle, id: &str, label: &str) -> tauri::Result<Box<dyn IsMenuItem<Wry>>> {
    Ok(Box::new(MenuItem::with_id(
        app,
        id,
        label,
        false,
        None::<&str>,
    )?))
}

fn save_item(app: &AppHandle, save: &Save) -> tauri::Result<Box<dyn IsMenuItem<Wry>>> {
    let raw = if save.title.trim().is_empty() {
        save.url
            .trim_start_matches("https://")
            .trim_start_matches("http://")
            .trim_start_matches("www.")
    } else {
        save.title.trim()
    };
    let mut title: String = raw.chars().take(TITLE_MAX_CHARS).collect();
    if raw.chars().count() > TITLE_MAX_CHARS {
        title.push('…');
    }
    Ok(Box::new(MenuItem::with_id(
        app,
        format!("url:{}", save.url),
        title,
        true,
        None::<&str>,
    )?))
}

pub fn show_main(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

fn toggle_quick_panel(app: &AppHandle, cursor: PhysicalPosition<f64>) {
    let Some(window) = app.get_webview_window(QUICK_WINDOW) else {
        return;
    };
    let guard = app.state::<QuickPanelGuard>();
    if window.is_visible().unwrap_or(false) || guard.hidden_just_now() {
        let _ = window.hide();
        return;
    }
    position_near_tray(app, &window, cursor);
    let _ = window.show();
    let _ = window.set_focus();
}

/// Place the panel near the tray click: below it when the tray sits in the
/// top half of the screen, above it when in the bottom half (Windows
/// taskbar). Clamped to the monitor horizontally.
fn position_near_tray(app: &AppHandle, window: &WebviewWindow, cursor: PhysicalPosition<f64>) {
    let size = match window.outer_size() {
        Ok(s) => s,
        Err(_) => return,
    };
    let monitor = app
        .monitor_from_point(cursor.x, cursor.y)
        .ok()
        .flatten()
        .or_else(|| app.primary_monitor().ok().flatten());
    let Some(monitor) = monitor else { return };

    let scale = monitor.scale_factor();
    let margin = EDGE_MARGIN * scale;
    let mon_pos = monitor.position();
    let mon_size = monitor.size();

    let mut x = cursor.x - size.width as f64 / 2.0;
    x = x.clamp(
        mon_pos.x as f64 + margin,
        mon_pos.x as f64 + mon_size.width as f64 - size.width as f64 - margin,
    );

    let tray_in_top_half = cursor.y < mon_pos.y as f64 + mon_size.height as f64 / 2.0;
    let y = if tray_in_top_half {
        // Hug the menubar so the panel's arrow notch points at the icon.
        cursor.y + 6.0 * scale
    } else {
        cursor.y - size.height as f64 - margin * 2.0
    };

    let _ = window.set_position(PhysicalPosition::new(x.round() as i32, y.round() as i32));
}
