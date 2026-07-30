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

import 'package:flutter/material.dart';
import 'package:flutter_rust_bridge/flutter_rust_bridge_for_generated.dart'
    show ExternalLibrary;
import 'package:path_provider/path_provider.dart';

import 'app.dart';
import 'src/rust/api/vault_api.dart';
import 'src/rust/frb_generated.dart';

/// Where the Tauri app keeps its vault, so both frontends see one dataset.
/// SQLite is in WAL mode, so having both open at once is fine — the macOS
/// menubar app already relies on that.
Future<String> tauriVaultPath() async {
  // Lets the probe be pointed at a scratch copy of the vault.
  const override = String.fromEnvironment('WEBSAVE_VAULT');
  if (override.isNotEmpty) return override;
  final support = await getApplicationSupportDirectory();
  final tauriDir = Directory(
    '${support.parent.path}/com.venugopal.web-save/vault',
  );
  if (tauriDir.existsSync()) return tauriDir.path;
  // Fall back to our own sandbox so a fresh machine still runs.
  return '${support.path}/vault';
}

/// Locate the dev build of the Rust library.
///
/// `Directory.current` is the repo when run via `flutter run`, but `/` when
/// the built .app is launched from Finder — so try both, and let
/// `--dart-define=WEBSAVE_DYLIB=…` override. Once cargokit bundles the
/// library into the app (Phase 4) this whole function goes away.
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

  // A probe that dies before runApp() shows a black window and tells you
  // nothing, so every failure is caught and rendered instead.
  try {
    final libPath = findRustLibrary();
    if (libPath == null) {
      throw StateError(
        'Could not find libwebsave_bridge.dylib.\n'
        'Run `cargo build -p websave-bridge` from the repo root, or pass\n'
        '--dart-define=WEBSAVE_DYLIB=/absolute/path/to/libwebsave_bridge.dylib',
      );
    }
    await RustLib.init(externalLibrary: ExternalLibrary.open(libPath));
    final opened = await openVault(path: await tauriVaultPath());
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

