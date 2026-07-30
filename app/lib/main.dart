// Phase 0 of the Flutter migration: prove the bridge.
//
// This opens the *real* vault the Tauri app uses and renders it. It is
// deliberately not a port of the UI — no theming, no sidebar, no editing.
// The only question it answers is whether Dart can talk to `websave-core`
// against real data, which is the thing every later phase depends on.
//
// Dev-time note: the Rust library is loaded from `target/` by absolute path
// rather than bundled. Wiring cargokit into the Xcode build is a Phase 4
// job and would only be in the way right now.

import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter_rust_bridge/flutter_rust_bridge_for_generated.dart'
    show ExternalLibrary;
import 'package:path_provider/path_provider.dart';

import 'src/rust/api/vault_api.dart';
import 'src/rust/frb_generated.dart';

/// Where the Tauri app keeps its vault, so both frontends see one dataset.
/// SQLite is in WAL mode, so having both open at once is fine — the macOS
/// menubar app already relies on that.
Future<String> tauriVaultPath() async {
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
    runApp(WebSaveProbe(vaultPath: opened));
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

class WebSaveProbe extends StatelessWidget {
  const WebSaveProbe({super.key, required this.vaultPath});
  final String vaultPath;

  @override
  Widget build(BuildContext context) {
    return MaterialApp(
      title: 'WebSave (Flutter probe)',
      theme: ThemeData(
        colorScheme: ColorScheme.fromSeed(seedColor: const Color(0xFF4C7DF0)),
        useMaterial3: true,
      ),
      home: SaveListScreen(vaultPath: vaultPath),
    );
  }
}

class SaveListScreen extends StatefulWidget {
  const SaveListScreen({super.key, required this.vaultPath});
  final String vaultPath;

  @override
  State<SaveListScreen> createState() => _SaveListScreenState();
}

class _SaveListScreenState extends State<SaveListScreen> {
  String _query = '';
  late Future<(List<SaveView>, StatsView)> _data;

  @override
  void initState() {
    super.initState();
    _load();
  }

  void _load() {
    _data = Future.wait([
      listSaves(query: _query.isEmpty ? null : _query, limit: 500),
      vaultStats(),
    ]).then((r) => (r[0] as List<SaveView>, r[1] as StatsView));
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(
        title: const Text('WebSave — Flutter probe'),
        bottom: PreferredSize(
          preferredSize: const Size.fromHeight(52),
          child: Padding(
            padding: const EdgeInsets.fromLTRB(12, 0, 12, 8),
            child: TextField(
              decoration: const InputDecoration(
                hintText: 'Search the real vault (FTS5)…',
                prefixIcon: Icon(Icons.search),
                isDense: true,
                border: OutlineInputBorder(),
              ),
              onChanged: (value) => setState(() {
                _query = value;
                _load();
              }),
            ),
          ),
        ),
      ),
      body: FutureBuilder<(List<SaveView>, StatsView)>(
        future: _data,
        builder: (context, snapshot) {
          if (snapshot.hasError) {
            return Center(child: Text('Rust error: ${snapshot.error}'));
          }
          if (!snapshot.hasData) {
            return const Center(child: CircularProgressIndicator());
          }
          final (saves, stats) = snapshot.data!;
          return Column(
            children: [
              _StatsBar(stats: stats, vaultPath: widget.vaultPath),
              const Divider(height: 1),
              Expanded(
                child: saves.isEmpty
                    ? const Center(child: Text('No saves matched.'))
                    : ListView.separated(
                        itemCount: saves.length,
                        separatorBuilder: (_, _) => const Divider(height: 1),
                        itemBuilder: (context, i) => _SaveTile(save: saves[i]),
                      ),
              ),
            ],
          );
        },
      ),
    );
  }
}

class _StatsBar extends StatelessWidget {
  const _StatsBar({required this.stats, required this.vaultPath});
  final StatsView stats;
  final String vaultPath;

  @override
  Widget build(BuildContext context) {
    final style = Theme.of(context).textTheme.bodySmall;
    return Padding(
      padding: const EdgeInsets.all(12),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Wrap(
            spacing: 16,
            children: [
              Text('${stats.total} saves', style: style),
              Text('${stats.favorites} starred', style: style),
              Text('${stats.unread} unread', style: style),
              Text('${stats.active} active', style: style),
              Text('${stats.changed} changed', style: style),
              Text('${stats.dead} dead', style: style),
            ],
          ),
          const SizedBox(height: 4),
          Text(vaultPath, style: style?.copyWith(color: Colors.grey)),
        ],
      ),
    );
  }
}

class _SaveTile extends StatelessWidget {
  const _SaveTile({required this.save});
  final SaveView save;

  /// Hardcoded here on purpose: in the real app these five become tokens on
  /// the theme model, so every pack can restate them.
  static const _statusColors = <String, Color>{
    'active': Color(0xFF3BA55D),
    'changed': Color(0xFFE0A33E),
    'redirected': Color(0xFF4C7DF0),
    'dead': Color(0xFFD64545),
    'unchecked': Color(0xFF9AA0A6),
  };

  @override
  Widget build(BuildContext context) {
    return ListTile(
      leading: Container(
        width: 10,
        height: 10,
        decoration: BoxDecoration(
          shape: BoxShape.circle,
          color: _statusColors[save.status] ?? Colors.grey,
        ),
      ),
      title: Text(
        save.title.isEmpty ? save.url : save.title,
        maxLines: 1,
        overflow: TextOverflow.ellipsis,
      ),
      subtitle: Text(
        Uri.tryParse(save.url)?.host ?? save.url,
        maxLines: 1,
        overflow: TextOverflow.ellipsis,
      ),
      trailing: Wrap(
        spacing: 8,
        crossAxisAlignment: WrapCrossAlignment.center,
        children: [
          if (save.favorite) const Icon(Icons.star, size: 16),
          if (!save.isRead)
            const Icon(Icons.circle, size: 8, color: Color(0xFF4C7DF0)),
          Text(save.status, style: Theme.of(context).textTheme.labelSmall),
        ],
      ),
    );
  }
}
