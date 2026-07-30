// The app above the bridge: theme engine, shells, destinations.
//
// Everything platform-specific is absent on purpose. The engine is created
// once, the shell picks itself by width, and the feature screens below know
// about neither.

import 'package:flutter/material.dart';
import 'package:flutter_theme_engine/flutter_theme_engine.dart';

import 'features/saves_controller.dart';
import 'features/saves_screen.dart';
import 'features/settings_screen.dart';
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
      sidebar: (context, index, onSelect) => ListenableBuilder(
        listenable: _saves,
        builder: (context, _) => VaultSidebar(
          controller: _saves,
          trailing: const ThemeQuickPicker(),
          // Settings is the last destination.
          onOpenSettings: () => onSelect(2),
        ),
      ),
      destinations: [
        ShellDestination(
          id: 'saves',
          label: 'All Saves',
          icon: Icons.bookmark_outline,
          builder: (_) => SavesScreen(controller: _saves),
        ),
        ShellDestination(
          id: 'map',
          label: 'Served From',
          icon: Icons.public,
          builder: (_) => const WorldMapScreen(),
        ),
        ShellDestination(
          id: 'settings',
          label: 'Settings',
          icon: Icons.tune,
          builder: (_) => SettingsScreen(vaultPath: widget.vaultPath),
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
            selected: engine.currentId == pack.id ||
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
