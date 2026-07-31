// Bootstrapping: find the Rust library, open the vault, build the theme
// engine, hand off to app.dart.
//
// Everything UI lives elsewhere — this file only exists to get those three
// things wired before first paint.
//
// Dev-time note: the Rust library is loaded from `target/` by absolute path
// rather than bundled. Wiring cargokit into the Xcode build is a Phase 4
// job and would only be in the way right now.

import 'dart:async';
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
/// The Rust library backing this app.
///
/// Cargokit compiles the crate as a static library and links it into the
/// executable with -force_load on every platform, so by the time Dart runs
/// the symbols are already in the process — there is no file to find and
/// nothing to bundle separately. This replaced a loader that searched the
/// source tree for a dylib, which only ever worked when the app was launched
/// from inside the repo and silently picked up a stale debug build otherwise.
ExternalLibrary rustLibrary() => ExternalLibrary.process(iKnowHowToUseIt: true);

Future<void> main() async {
  WidgetsFlutterBinding.ensureInitialized();
  await setUpDesktopWindow();
  // Paint *first*, then initialise. Awaiting anything before runApp means a
  // step that hangs rather than throws shows a blank screen and nothing else
  // — which is exactly what the first iPad build did, with no VM service
  // reachable to explain it. Now the device reports its own startup.
  runApp(const Bootstrap());
}

/// Runs startup with each step visible, and turns a hang into a message.
class Bootstrap extends StatefulWidget {
  const Bootstrap({super.key});

  @override
  State<Bootstrap> createState() => _BootstrapState();
}

class _BootstrapState extends State<Bootstrap> {
  /// Generous, but finite. Every step here is local work — no step should
  /// take seconds, so anything that does is stuck rather than slow.
  static const _stepTimeout = Duration(seconds: 20);

  String _step = 'Starting…';
  Widget? _app;
  String? _error;
  String? _stack;

  @override
  void initState() {
    super.initState();
    _start();
  }

  Future<T> _run<T>(String label, Future<T> Function() step) async {
    setState(() => _step = label);
    return step().timeout(
      _stepTimeout,
      onTimeout: () => throw TimeoutException(
        '$label timed out after ${_stepTimeout.inSeconds}s',
      ),
    );
  }

  Future<void> _start() async {
    try {
      await _run('Loading the engine…', () async {
        await RustLib.init(externalLibrary: rustLibrary());
      });
      final path = await _run('Finding the vault…', vaultPath);
      final opened = await _run(
        'Opening the vault…',
        () => openVault(path: path),
      );
      // The engine reads its saved selection from the vault, so it can only
      // be built once the vault is open.
      final engine = await _run('Restoring the theme…', createThemeEngine);
      if (!mounted) return;
      setState(() => _app = WebSaveApp(engine: engine, vaultPath: opened));
    } catch (e, stack) {
      if (!mounted) return;
      setState(() {
        _error = '$e';
        _stack = '$stack';
      });
    }
  }

  @override
  Widget build(BuildContext context) {
    if (_app != null) return _app!;
    if (_error != null) {
      return StartupFailure(message: _error!, stack: _stack ?? '');
    }
    return MaterialApp(
      debugShowCheckedModeBanner: false,
      home: Scaffold(
        body: Center(
          child: Column(
            mainAxisSize: MainAxisSize.min,
            children: [
              const CircularProgressIndicator(),
              const SizedBox(height: 16),
              Text(_step, style: const TextStyle(fontSize: 13)),
            ],
          ),
        ),
      ),
    );
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
