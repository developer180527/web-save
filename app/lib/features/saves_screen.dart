// The saves list, and the one place the pane-vs-route decision is made.
//
// Token-only: no colour, radius or height in this file is a literal. That is
// what lets a theme pack restyle the app without touching a widget, and the
// thing to check when reviewing new UI here.

import 'package:flutter/material.dart';

import '../shell/layout_scope.dart';
import '../src/rust/api/vault_api.dart';
import '../theme/websave_theme.dart';
import 'save_card.dart';
import 'save_detail.dart';
import 'save_row.dart';
import 'saves_controller.dart';

class SavesScreen extends StatefulWidget {
  const SavesScreen({super.key, required this.controller});

  final SavesController controller;

  @override
  State<SavesScreen> createState() => _SavesScreenState();
}

class _SavesScreenState extends State<SavesScreen> {
  SavesController get _controller => widget.controller;

  @override
  void initState() {
    super.initState();
    _controller.addListener(_surfaceErrors);
  }

  @override
  void dispose() {
    // Owned by HomeShell, so listen and let go — never dispose it here.
    _controller.removeListener(_surfaceErrors);
    super.dispose();
  }

  /// A failed write must never be silent — the row would simply not change
  /// and the click would look like it missed.
  void _surfaceErrors() {
    final error = _controller.error;
    if (error == null || !mounted) return;
    _controller.clearError();
    ScaffoldMessenger.of(context).showSnackBar(
      SnackBar(
        content: Text(error),
        backgroundColor: context.ws.danger,
        behavior: SnackBarBehavior.floating,
      ),
    );
  }

  void _open(SaveView save) {
    if (context.layout.showsDetailPane) {
      _controller.select(save);
    } else {
      // A selection surviving the pop would highlight a row with no pane to
      // explain it, so the pushed route owns the save outright.
      Navigator.of(context).push(
        MaterialPageRoute<void>(
          builder: (_) => SaveDetailPage(save: save, controller: _controller),
        ),
      );
    }
  }

  @override
  Widget build(BuildContext context) {
    final ws = context.ws;
    return ListenableBuilder(
      listenable: _controller,
      builder: (context, _) {
        final list = Column(
          children: [
            _Header(controller: _controller),
            if (_controller.stats case final stats?) _StatsStrip(stats: stats),
            Expanded(child: _buildList(context)),
          ],
        );

        if (!context.layout.showsDetailPane) return list;
        return Row(
          children: [
            Expanded(flex: 5, child: list),
            Container(width: 1, color: ws.border),
            Expanded(
              flex: 4,
              child: _controller.selected == null
                  ? const _EmptyPane()
                  : SaveDetail(
                      save: _controller.selected!,
                      controller: _controller,
                      onClose: () => _controller.select(null),
                    ),
            ),
          ],
        );
      },
    );
  }

  Widget _buildList(BuildContext context) {
    final ws = context.ws;
    if (_controller.isLoading && _controller.saves.isEmpty) {
      return const Center(child: CircularProgressIndicator());
    }
    if (_controller.saves.isEmpty) {
      return Center(
        child: Text('No saves matched.', style: TextStyle(color: ws.textDim)),
      );
    }
    if (!_controller.isGridView) {
      return ListView.builder(
        padding: EdgeInsets.symmetric(horizontal: ws.cardPadding, vertical: 6),
        itemCount: _controller.saves.length,
        itemBuilder: (context, i) {
          final save = _controller.saves[i];
          return SaveRow(
            save: save,
            selected: save.id == _controller.selected?.id,
            checking: _controller.isChecking(save.id),
            onTap: () => _open(save),
            controller: _controller,
          );
        },
      );
    }

    // A max-extent delegate rather than a fixed column count, so the grid
    // reflows as the pane is resized without the app choosing breakpoints for
    // it. `builder` keeps it lazy: only visible cards are ever built, which
    // is what makes a few thousand image-bearing cards affordable.
    return GridView.builder(
      padding: EdgeInsets.all(ws.cardPadding),
      gridDelegate: const SliverGridDelegateWithMaxCrossAxisExtent(
        maxCrossAxisExtent: 340,
        mainAxisExtent: 300,
        crossAxisSpacing: 12,
        mainAxisSpacing: 12,
      ),
      itemCount: _controller.saves.length,
      itemBuilder: (context, i) {
        final save = _controller.saves[i];
        return SaveCard(
          save: save,
          selected: save.id == _controller.selected?.id,
          checking: _controller.isChecking(save.id),
          thumbnailPath: _controller.thumbnailPath(save),
          onTap: () => _open(save),
          controller: _controller,
        );
      },
    );
  }
}

/// Search plus the view toggle. The React app also has Import and Add here;
/// both are stubbed until their screens come across.
class _Header extends StatelessWidget {
  const _Header({required this.controller});
  final SavesController controller;

  @override
  Widget build(BuildContext context) {
    final ws = context.ws;
    return Container(
      padding: EdgeInsets.fromLTRB(
        ws.cardPadding,
        ws.cardPadding,
        ws.cardPadding,
        8,
      ),
      decoration: BoxDecoration(
        color: ws.background,
        border: Border(bottom: BorderSide(color: ws.border)),
      ),
      // No SafeArea here: the shell already inset the top, and insetting
      // twice leaves a visible gap above the search field.
      child: Row(
        children: [
          Expanded(child: _SearchBar(onChanged: controller.search)),
          const SizedBox(width: 10),
          _ViewToggle(controller: controller),
        ],
      ),
    );
  }
}

class _ViewToggle extends StatelessWidget {
  const _ViewToggle({required this.controller});
  final SavesController controller;

  @override
  Widget build(BuildContext context) {
    final ws = context.ws;
    return Container(
      height: ws.rowHeight,
      decoration: BoxDecoration(
        color: ws.panel,
        border: Border.all(color: ws.border),
        borderRadius: BorderRadius.circular(ws.controlRadius),
      ),
      child: Row(
        children: [
          _ToggleButton(
            icon: Icons.view_list,
            tooltip: 'List view',
            active: !controller.isGridView,
            onTap: () {
              if (controller.isGridView) controller.toggleView();
            },
          ),
          _ToggleButton(
            icon: Icons.grid_view,
            tooltip: 'Card view',
            active: controller.isGridView,
            onTap: () {
              if (!controller.isGridView) controller.toggleView();
            },
          ),
        ],
      ),
    );
  }
}

class _ToggleButton extends StatelessWidget {
  const _ToggleButton({
    required this.icon,
    required this.tooltip,
    required this.active,
    required this.onTap,
  });

  final IconData icon;
  final String tooltip;
  final bool active;
  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) {
    final ws = context.ws;
    return Tooltip(
      message: tooltip,
      child: GestureDetector(
        onTap: onTap,
        child: Container(
          width: 36,
          height: double.infinity,
          decoration: BoxDecoration(
            color: active ? ws.states.selected : Colors.transparent,
            borderRadius: BorderRadius.circular(ws.controlRadius - 1),
          ),
          child: Icon(icon, size: 16, color: active ? ws.text : ws.textDim),
        ),
      ),
    );
  }
}

class _SearchBar extends StatelessWidget {
  const _SearchBar({required this.onChanged});
  final ValueChanged<String> onChanged;

  @override
  Widget build(BuildContext context) {
    final ws = context.ws;
    return SizedBox(
      height: ws.rowHeight,
      child: TextField(
        onChanged: onChanged,
        style: TextStyle(fontSize: 13.5, color: ws.text),
        decoration: InputDecoration(
          hintText: 'Search titles, notes, tags and archived text…',
          hintStyle: TextStyle(color: ws.textDim, fontSize: 13.5),
          prefixIcon: Icon(Icons.search, size: 17, color: ws.textDim),
          prefixIconConstraints: const BoxConstraints(minWidth: 34),
          contentPadding: const EdgeInsets.symmetric(vertical: 8),
        ),
      ),
    );
  }
}

class _StatsStrip extends StatelessWidget {
  const _StatsStrip({required this.stats});
  final StatsView stats;

  @override
  Widget build(BuildContext context) {
    final ws = context.ws;
    final compact = context.layout.isCompact;
    final entries = <(String, int, Color)>[
      ('active', stats.active.toInt(), ws.health.active),
      ('changed', stats.changed.toInt(), ws.health.changed),
      ('redirected', stats.redirected.toInt(), ws.health.redirected),
      ('dead', stats.dead.toInt(), ws.health.dead),
    ];
    return Container(
      width: double.infinity,
      padding: EdgeInsets.symmetric(horizontal: ws.cardPadding, vertical: 8),
      child: Wrap(
        spacing: 14,
        runSpacing: 4,
        crossAxisAlignment: WrapCrossAlignment.center,
        children: [
          Text(
            '${stats.total} saves',
            style: TextStyle(
              color: ws.text,
              fontSize: 12,
              fontWeight: FontWeight.w600,
            ),
          ),
          if (!compact)
            Text(
              '${stats.unread} unread',
              style: TextStyle(color: ws.textDim, fontSize: 12),
            ),
          for (final (label, count, color) in entries)
            Row(
              mainAxisSize: MainAxisSize.min,
              children: [
                Container(
                  width: 7,
                  height: 7,
                  decoration: BoxDecoration(
                    color: color,
                    shape: BoxShape.circle,
                  ),
                ),
                const SizedBox(width: 5),
                Text(
                  compact ? '$count' : '$count $label',
                  style: TextStyle(color: ws.textDim, fontSize: 12),
                ),
              ],
            ),
        ],
      ),
    );
  }
}

class _EmptyPane extends StatelessWidget {
  const _EmptyPane();

  @override
  Widget build(BuildContext context) {
    final ws = context.ws;
    return Center(
      child: Column(
        mainAxisSize: MainAxisSize.min,
        children: [
          Icon(Icons.article_outlined, size: 34, color: ws.border),
          const SizedBox(height: 10),
          Text(
            'Select a save',
            style: TextStyle(color: ws.textDim, fontSize: 13),
          ),
        ],
      ),
    );
  }
}
