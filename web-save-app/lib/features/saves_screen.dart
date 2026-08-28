// The saves list, and the one place the pane-vs-route decision is made.
//
// Token-only: no colour, radius or height in this file is a literal. That is
// what lets a theme pack restyle the app without touching a widget, and the
// thing to check when reviewing new UI here.

import '../theme/app_icons.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

import '../shell/layout_scope.dart';
import '../shell/recessed_well.dart';
import '../src/rust/api/vault_api.dart';
import '../theme/websave_theme.dart';
import 'save_card.dart';
import 'save_detail.dart';
import 'add_save_sheet.dart';
import 'save_row.dart';
import 'saves_controller.dart';

class SavesScreen extends StatefulWidget {
  const SavesScreen({super.key, required this.controller});

  final SavesController controller;

  @override
  State<SavesScreen> createState() => _SavesScreenState();
}

class _SavesScreenState extends State<SavesScreen> with WidgetsBindingObserver {
  SavesController get _controller => widget.controller;

  @override
  void initState() {
    super.initState();
    _controller.addListener(_surfaceErrors);
    WidgetsBinding.instance.addObserver(this);
    _controller.startPolling();
    // Also on cold start, which resume alone does not cover.
    WidgetsBinding.instance.addPostFrameCallback((_) => _offerClipboard());
  }

  /// The last clipboard link we offered, so returning to the app repeatedly
  /// with the same link on the clipboard does not nag.
  String? _offered;

  @override
  void didChangeAppLifecycleState(AppLifecycleState state) {
    if (state == AppLifecycleState.resumed) {
      _controller.startPolling();
      // Catch up immediately rather than waiting out a tick — coming back to
      // the window is exactly when a save made elsewhere should already be
      // there.
      _controller.load(quiet: true);
      _offerClipboard();
    } else {
      _controller.stopPolling();
    }
  }

  /// Offer to save a link sitting on the clipboard.
  ///
  /// A bar, not a dialog: coming back to the app to do something else and
  /// being interrupted by a modal about a link you copied for another reason
  /// is the behaviour that makes people distrust clipboard access. This can be
  /// ignored by doing nothing.
  Future<void> _offerClipboard() async {
    try {
      final data = await Clipboard.getData(Clipboard.kTextPlain);
      final text = data?.text?.trim();
      if (text == null || text.isEmpty || text == _offered) return;
      if (!await looksLikeUrl(text: text)) return;
      // Already saved: offering it again is noise.
      if (_controller.saves.any((s) => s.url == text)) return;
      if (!mounted) return;
      _offered = text;
      final ws = context.ws;
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(
          backgroundColor: ws.panel,
          duration: const Duration(seconds: 8),
          content: Text(
            'Link on your clipboard: $text',
            maxLines: 1,
            overflow: TextOverflow.ellipsis,
            style: TextStyle(color: ws.text, fontSize: 12.5),
          ),
          action: SnackBarAction(
            label: 'Paste & Add',
            textColor: ws.accent,
            onPressed: () => _controller.addByUrl(text),
          ),
        ),
      );
    } catch (_) {
      // An unreadable clipboard is not worth reporting; the add button works.
    }
  }

  @override
  void dispose() {
    _controller.stopPolling();
    WidgetsBinding.instance.removeObserver(this);
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

  /// Double-click: hand the link to the browser and take it out of the inbox.
  ///
  /// Opening is what a save is *for*, so it counts as reading it. Marking is
  /// one-way — [SavesController.markRead] rather than a toggle — because
  /// opening the same link twice should not put it back in the inbox.
  void _launch(SaveView save) {
    openInBrowser(save.url);
    _controller.markRead(save);
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
    // Three regions, three subscriptions. The list is the expensive one —
    // every visible card and its thumbnail — so it must not repaint when the
    // counts tick over or when a row is selected in the pane beside it.
    final list = Column(
      children: [
        _Header(controller: _controller),
        ValueListenableBuilder<StatsView?>(
          valueListenable: _controller.statsView,
          builder: (context, stats, _) => stats == null
              ? const SizedBox.shrink()
              : _StatsStrip(stats: stats),
        ),
        // Only the results are recessed. The header and the stats strip above
        // are chrome, on the same plane as the sidebar and the title bar, and
        // the lip is drawn where they meet the well.
        Expanded(
          child: RecessedWell(
            // The sidebar is a wall to the left of the well on desktop; on a
            // phone the well runs to the window edge with nothing above it to
            // cast, and shading an edge with no wall reads as a vignette.
            shadeLeading: context.layout.showsSidebar,
            child: ListenableBuilder(
              listenable: Listenable.merge([_controller, _controller.gridView]),
              builder: (context, _) => _buildList(context),
            ),
          ),
        ),
      ],
    );

    if (!context.layout.showsDetailPane) return list;

    // The pane is only worth its width when it has something to show. It used
    // to sit there permanently saying "Select a save", which spent nearly half
    // the window on an instruction and left the grid two columns narrower than
    // it needed to be.
    return ValueListenableBuilder<SaveView?>(
      valueListenable: _controller.selection,
      builder: (context, selected, _) {
        if (selected == null) return list;
        return Row(
          children: [
            Expanded(flex: 5, child: list),
            Container(width: 1, color: ws.border),
            Expanded(
              flex: 4,
              child: SaveDetail(
                save: selected,
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
            // Only where a single click *previews* rather than navigates.
            // On touch the tap already opens the save full screen, and a
            // double tap there would fight the scroll gesture.
            onDoubleTap: context.layout.showsDetailPane
                ? () => _launch(save)
                : null,
            controller: _controller,
          );
        },
      );
    }

    // Cards are a fixed width; only the number of columns changes.
    //
    // This flexed once, stretching cards to consume every point of the pane,
    // which meant dragging the window edge resized every visible card and
    // relaid out every thumbnail. Holding the width still and reflowing only
    // the column count is what the Tauri app does, and it is the reason its
    // grid reads as steady while this one read as elastic.
    //
    // The tradeoff is a margin: a pane sitting between two column boundaries
    // leaves up to a card's width unused. That is the price of the cards not
    // moving, and it is worth paying.
    //
    // `builder` still only builds what is visible, so a few thousand
    // image-bearing cards stay affordable.
    return LayoutBuilder(
      builder: (context, constraints) {
        const gap = 12.0;

        final margin = ws.cardPadding;
        final available = constraints.maxWidth - margin * 2;

        // How many whole cards fit. At least one even in a pane narrower than
        // a card, where the card is allowed to be the thing that overflows
        // rather than the layout breaking.
        final columns = ((available + gap) / (SaveCard.width + gap))
            .floor()
            .clamp(1, 12);

        // Constant, unlike before. The leftover width becomes margin.
        const cardWidth = SaveCard.width;
        final gridWidth = columns * cardWidth + (columns - 1) * gap;

        return Align(
          alignment: Alignment.topCenter,
          child: SizedBox(
            width: gridWidth,
            child: GridView.builder(
              padding: EdgeInsets.symmetric(vertical: ws.cardPadding),
              gridDelegate: SliverGridDelegateWithFixedCrossAxisCount(
                crossAxisCount: columns,
                // Derived from the width the cards actually got, so the tile
                // is exactly as tall as the card wants to be at that width.
                mainAxisExtent: SaveCard.heightFor(cardWidth),
                crossAxisSpacing: gap,
                mainAxisSpacing: gap,
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
                  onDoubleTap: context.layout.showsDetailPane
                      ? () => _launch(save)
                      : null,
                  controller: _controller,
                );
              },
            ),
          ),
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
          // Every platform needs this: the browser extension is the only
          // other way in, and on iOS there is no extension at all.
          IconButton(
            onPressed: () => showAddSaveSheet(context, controller),
            icon: Icon(AppIcons.add, size: 19),
            tooltip: 'Add a link',
            color: context.ws.text,
          ),
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
  Widget build(BuildContext context) => ValueListenableBuilder<bool>(
    valueListenable: controller.gridView,
    builder: (context, _, _) => _build(context),
  );

  Widget _build(BuildContext context) {
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
            icon: AppIcons.listView,
            tooltip: 'List view',
            active: !controller.isGridView,
            onTap: () {
              if (controller.isGridView) controller.toggleView();
            },
          ),
          _ToggleButton(
            icon: AppIcons.gridView,
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
          prefixIcon: Icon(AppIcons.search, size: 17, color: ws.textDim),
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
