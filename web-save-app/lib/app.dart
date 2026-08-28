// The app above the bridge: theme engine, shells, destinations.
//
// Everything platform-specific is absent on purpose. The engine is created
// once, the shell picks itself by width, and the feature screens below know
// about neither.

import 'theme/app_icons.dart';
import 'package:flutter/material.dart';
import 'package:flutter_theme_engine/flutter_theme_engine.dart';

import 'features/saves_controller.dart';
import 'features/saves_screen.dart';
import 'features/settings_screen.dart';
import 'features/transfer_screen.dart';
import 'features/vault_sidebar.dart';
import 'shell/app_shell.dart';
import 'theme/packs.dart';
import 'theme/pointer_density_trigger.dart';
import 'theme/vault_theme_store.dart';
import 'theme/websave_theme.dart';
import 'world_map.dart';

/// Build and restore the engine. Call once, before `runApp`.
Future<ThemeEngine<WebSaveTheme>> createThemeEngine() async {
  final engine = ThemeEngine<WebSaveTheme>(store: const VaultThemeStore())
    ..register(webSavePacks)
    ..addTrigger(const UserSelectionTrigger())
    ..addTrigger(const PointerDensityTrigger())
    ..addTrigger(
      const PlatformBrightnessTrigger(lightId: 'daylight', darkId: 'midnight'),
    );
  // Recover the previous choice before first paint, so the app never flashes
  // the fallback theme on launch.
  await engine.restore();
  return engine;
}

class WebSaveApp extends StatelessWidget {
  const WebSaveApp({super.key, required this.engine, required this.vaultPath});

  final ThemeEngine<WebSaveTheme> engine;
  final String vaultPath;

  @override
  Widget build(BuildContext context) {
    return ThemeEngineBuilder<WebSaveTheme>(
      engine: engine,
      builder: (context, pack) => MaterialApp(
        title: 'WebSave',
        debugShowCheckedModeBanner: false,
        theme: pack.themeData,
        // Respect the OS "reduce motion" setting: crossfading a grid of a few
        // hundred cards is not cheap, and it is exactly the sort of animation
        // people turn that setting off for.
        themeAnimationDuration: MediaQuery.disableAnimationsOf(context)
            ? Duration.zero
            : const Duration(milliseconds: 180),
        home: HomeShell(vaultPath: vaultPath),
      ),
    );
  }
}

class HomeShell extends StatefulWidget {
  const HomeShell({super.key, required this.vaultPath});

  final String vaultPath;

  @override
  State<HomeShell> createState() => _HomeShellState();
}

class _HomeShellState extends State<HomeShell> {
  // Owned here rather than inside SavesScreen so the sidebar's counts,
  // filters and tags read the same state the list does.
  final _saves = SavesController();

  @override
  void initState() {
    super.initState();
    _saves.load();
  }

  @override
  void dispose() {
    _saves.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return AppShell(
      // Listens to the narrowing and the counts, not to the controller as a
      // whole. Bound to the controller it rebuilt on every list change too,
      // so a filter tap repainted the grid before the highlight could move.
      sidebar: (context, index, onSelect) => ListenableBuilder(
        listenable: Listenable.merge([
          _saves.criteria,
          _saves.statsView,
          _saves.tagCounts,
        ]),
        builder: (context, _) => VaultSidebar(
          controller: _saves,
          trailing: const ThemeQuickPicker(),
          // Indices into `destinations` below. Settings is no longer last —
          // Transfer sits after it — so these are pinned to position.
          onOpenSettings: () => onSelect(2),
          onShowSaves: () => onSelect(0),
        ),
      ),
      destinations: [
        ShellDestination(
          id: 'saves',
          label: 'All Saves',
          icon: AppIcons.bookmark,
          builder: (_) => SavesScreen(controller: _saves),
        ),
        ShellDestination(
          id: 'map',
          label: 'Served From',
          icon: AppIcons.reachable,
          builder: (_) => const WorldMapScreen(),
        ),
        ShellDestination(
          id: 'settings',
          label: 'Settings',
          icon: AppIcons.settings,
          builder: (_) =>
              SettingsScreen(vaultPath: widget.vaultPath, controller: _saves),
        ),
        ShellDestination(
          id: 'transfer',
          label: 'Import & Export',
          icon: AppIcons.transfer,
          builder: (_) => TransferScreen(controller: _saves),
        ),
      ],
      trailing: (context) => const ThemeQuickPicker(),
    );
  }
}

/// A compact swatch row in the sidebar footer.
///
/// Draws each pack from its stored `swatch` rather than from
/// `pack.themeData`, so opening a catalogue of forty themes never triggers
/// forty `ColorScheme.fromSeed` runs.
class ThemeQuickPicker extends StatelessWidget {
  const ThemeQuickPicker({super.key});

  @override
  Widget build(BuildContext context) {
    final engine = ThemeEngineScope.of<WebSaveTheme>(context);
    // Touch variants are a density mechanism, not a user-facing choice.
    final packs = engine.packs.where((p) => !p.id.endsWith('_touch')).toList();

    return Wrap(
      spacing: 6,
      runSpacing: 6,
      children: [
        for (final pack in packs)
          _Swatch(
            pack: pack,
            selected:
                engine.currentId == pack.id ||
                engine.currentId == '${pack.id}_touch',
            onTap: () => engine.apply(pack.id),
          ),
      ],
    );
  }
}

class _Swatch extends StatelessWidget {
  const _Swatch({
    required this.pack,
    required this.selected,
    required this.onTap,
  });

  final ThemePack<WebSaveTheme> pack;
  final bool selected;
  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) {
    final ws = context.ws;
    final colors = pack.model.swatch;
    return Tooltip(
      message: pack.meta.name,
      child: GestureDetector(
        onTap: onTap,
        child: Container(
          width: 30,
          height: 22,
          decoration: BoxDecoration(
            borderRadius: BorderRadius.circular(ws.iconTileRadius),
            border: Border.all(
              color: selected ? ws.accent : ws.border,
              width: selected ? 2 : 1,
            ),
          ),
          child: ClipRRect(
            borderRadius: BorderRadius.circular(ws.iconTileRadius - 1),
            child: Row(
              children: [
                for (final color in colors)
                  Expanded(child: Container(color: color)),
              ],
            ),
          ),
        ),
      ),
    );
  }
}
