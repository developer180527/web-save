// Bootstrapping: find the Rust library, open the vault, build the theme
// engine, hand off to app.dart.
//
// Everything UI lives elsewhere — this file only exists to get those three
// things wired before first paint.
//
// Dev-time note: the Rust library is loaded from `target/` by absolute path
// rather than bundled. Wiring cargokit into the Xcode build is a Phase 4
// job and would only be in the way right now.

import 'dart:io';

import 'package:flutter/foundation.dart' show kIsWeb;
import 'package:flutter/material.dart';
import 'package:flutter_rust_bridge/flutter_rust_bridge_for_generated.dart'
    show ExternalLibrary;
import 'package:path_provider/path_provider.dart';

import 'app.dart';
import 'platform/desktop_window.dart';
import 'src/rust/api/vault_api.dart';
import 'src/rust/frb_generated.dart';

/// Where this app's vault lives.
///
/// On desktop it prefers the Tauri app's vault so both frontends see one
/// dataset during the migration — SQLite is in WAL mode, and the macOS
/// menubar app already relies on that. On iOS there is no such vault and no
/// shared filesystem, so it gets its own inside the app container.
Future<String> vaultPath() async {
  const override = String.fromEnvironment('WEBSAVE_VAULT');
  if (override.isNotEmpty) return override;
  final support = await getApplicationSupportDirectory();
  if (!kIsWeb && !Platform.isIOS && !Platform.isAndroid) {
    final tauriDir = Directory(
      '${support.parent.path}/com.venugopal.web-save/vault',
    );
    if (tauriDir.existsSync()) return tauriDir.path;
  }
  return '${support.path}/vault';
}

/// How to reach the Rust symbols on this platform.
///
/// On iOS and Android the library is linked *into* the app binary — there is
/// no file to open, and its symbols are already in the running process. Only
/// desktop dev builds load a dylib from disk, and only because the library is
/// not bundled there yet.
ExternalLibrary rustLibrary() {
  if (!kIsWeb && (Platform.isIOS || Platform.isAndroid)) {
    return ExternalLibrary.process(iKnowHowToUseIt: true);
  }
  final path = findRustLibrary();
  if (path == null) {
    throw StateError(
      'Could not find libwebsave_bridge.dylib.\n'
      'Run `cargo build -p websave-bridge` from the repo root, or pass\n'
      '--dart-define=WEBSAVE_DYLIB=/absolute/path/to/libwebsave_bridge.dylib',
    );
  }
  return ExternalLibrary.open(path);
}

/// Locate the desktop dev build of the Rust library.
String? findRustLibrary() {
  const override = String.fromEnvironment('WEBSAVE_DYLIB');
  const name = 'libwebsave_bridge.dylib';
  final cwd = Directory.current.path;
  final candidates = <String>[
    if (override.isNotEmpty) override,
    '$cwd/target/debug/$name',
    '${Directory(cwd).parent.path}/target/debug/$name',
    // The .app launched from Finder: walk up from the bundle.
    '${File(Platform.resolvedExecutable).parent.parent.parent.parent.parent.parent.parent.parent.path}/target/debug/$name',
  ];
  return candidates.where((p) => File(p).existsSync()).firstOrNull;
}

Future<void> main() async {
  WidgetsFlutterBinding.ensureInitialized();
  await setUpDesktopWindow();

  // A probe that dies before runApp() shows a black window and tells you
  // nothing, so every failure is caught and rendered instead.
  try {
    await RustLib.init(externalLibrary: rustLibrary());
    final opened = await openVault(path: await vaultPath());
    // The engine reads its saved selection from the vault, so it can only be
    // built once the vault is open.
    final engine = await createThemeEngine();
    runApp(WebSaveApp(engine: engine, vaultPath: opened));
  } catch (e, stack) {
    runApp(StartupFailure(message: '$e', stack: '$stack'));
  }
}

class StartupFailure extends StatelessWidget {
  const StartupFailure({super.key, required this.message, required this.stack});
  final String message;
  final String stack;

  @override
  Widget build(BuildContext context) {
    return MaterialApp(
      home: Scaffold(
        body: Padding(
          padding: const EdgeInsets.all(24),
          child: SingleChildScrollView(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                const Text(
                  'Bridge startup failed',
                  style: TextStyle(fontSize: 20, fontWeight: FontWeight.bold),
                ),
                const SizedBox(height: 12),
                SelectableText(message),
                const SizedBox(height: 16),
                SelectableText(
                  stack,
                  style: const TextStyle(fontSize: 10, color: Colors.grey),
                ),
              ],
            ),
          ),
        ),
      ),
    );
  }
}

