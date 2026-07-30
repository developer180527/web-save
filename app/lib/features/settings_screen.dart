// Settings. Currently only the theme picker and vault info -- the rest comes
// across as the port continues.

import 'package:flutter/material.dart';
import 'package:flutter_theme_engine/flutter_theme_engine.dart';

import '../theme/websave_theme.dart';

class SettingsScreen extends StatelessWidget {
  const SettingsScreen({super.key, required this.vaultPath});

  final String vaultPath;

  @override
  Widget build(BuildContext context) {
    final ws = context.ws;
    final engine = ThemeEngineScope.of<WebSaveTheme>(context);
    final packs = engine.packs.where((p) => !p.id.endsWith('_touch')).toList();

    return ListView(
      padding: EdgeInsets.all(ws.cardPadding * 1.5),
      children: [
        Text(
          'Appearance',
          style: TextStyle(
            color: ws.text,
            fontSize: 15,
            fontWeight: FontWeight.w600,
          ),
        ),
        const SizedBox(height: 4),
        Text(
          'Themes are stored in the vault, so the choice travels with your '
          'data rather than living on this device.',
          style: TextStyle(color: ws.textDim, fontSize: 12.5),
        ),
        const SizedBox(height: 12),
        for (final pack in packs)
          _ThemeRow(
            pack: pack,
            selected: engine.currentId == pack.id ||
                engine.currentId == '${pack.id}_touch',
            onTap: () => engine.apply(pack.id),
          ),
        const SizedBox(height: 8),
        TextButton(
          onPressed: engine.reset,
          child: const Text('Follow the system appearance'),
        ),
        const SizedBox(height: 24),
        Text(
          'Vault',
          style: TextStyle(
            color: ws.text,
            fontSize: 15,
            fontWeight: FontWeight.w600,
          ),
        ),
        const SizedBox(height: 6),
        SelectableText(
          vaultPath,
          style: TextStyle(color: ws.textDim, fontSize: 12),
        ),
      ],
    );
  }
}

class _ThemeRow extends StatelessWidget {
  const _ThemeRow({
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
    return Padding(
      padding: const EdgeInsets.only(bottom: 6),
      child: GestureDetector(
        onTap: onTap,
        child: Container(
          padding: EdgeInsets.all(ws.cardPadding * 0.8),
          decoration: BoxDecoration(
            color: selected ? ws.states.selected : ws.panel,
            border: Border.all(color: selected ? ws.accent : ws.border),
            borderRadius: BorderRadius.circular(ws.cardRadius),
          ),
          child: Row(
            children: [
              // The stored swatch, never pack.themeData -- see the note on
              // WebSaveTheme.swatch.
              Container(
                width: 42,
                height: 28,
                decoration: BoxDecoration(
                  borderRadius: BorderRadius.circular(ws.iconTileRadius),
                  border: Border.all(color: ws.border),
                ),
                child: ClipRRect(
                  borderRadius: BorderRadius.circular(ws.iconTileRadius - 1),
                  child: Row(
                    children: [
                      for (final color in pack.model.swatch)
                        Expanded(child: Container(color: color)),
                    ],
                  ),
                ),
              ),
              const SizedBox(width: 12),
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text(
                      pack.meta.name,
                      style: TextStyle(
                        color: ws.text,
                        fontSize: 13.5,
                        fontWeight: FontWeight.w600,
                      ),
                    ),
                    if (pack.meta.description case final description?)
                      Text(
                        description,
                        style: TextStyle(color: ws.textDim, fontSize: 12),
                      ),
                  ],
                ),
              ),
              if (selected) Icon(Icons.check, size: 17, color: ws.accent),
            ],
          ),
        ),
      ),
    );
  }
}
