// A save as a card, with its cover image.
//
// Performance is the whole design here. A grid of a few thousand cards each
// showing a screenshot will drop frames for three separate reasons, and all
// three have to be handled or the view is unusable on a real vault:
//
//   1. Decoding. A 1200px screenshot decoded at full size costs megabytes of
//      RAM and a long rasterise. `cacheWidth` decodes at display size, which
//      is the single biggest win and the easiest to forget.
//   2. Rebuild scope. Hovering one card must not repaint the grid, so each
//      card is its own RepaintBoundary and owns its hover state.
//   3. Build scope. GridView.builder only builds what is visible, and the
//      fixed tile extent lets it place tiles without measuring them.

import 'dart:io';

import 'package:flutter/material.dart';

import '../src/rust/api/vault_api.dart';
import '../theme/websave_theme.dart';
import 'edit_save_sheet.dart';
import 'save_row.dart' show confirmDelete, openInBrowser;
import 'saves_controller.dart';

class SaveCard extends StatefulWidget {
  const SaveCard({
    super.key,
    required this.save,
    required this.selected,
    required this.onTap,
    required this.controller,
    this.thumbnailPath,
    this.checking = false,
  });

  final SaveView save;
  final bool selected;
  final VoidCallback onTap;
  final SavesController controller;
  final String? thumbnailPath;
  final bool checking;

  /// Constant, not flexible. Letting the cover absorb leftover space made
  /// every card's image a different height, because the text block below
  /// varies with description and tags — a constant cover is what makes the
  /// grid read as a grid.
  static const double coverHeight = 150;

  @override
  State<SaveCard> createState() => _SaveCardState();
}

class _SaveCardState extends State<SaveCard> {
  bool _hovered = false;

  @override
  Widget build(BuildContext context) {
    final ws = context.ws;
    final save = widget.save;
    final host = Uri.tryParse(save.url)?.host ?? save.url;

    // Isolates repaints: hovering one card cannot dirty its neighbours.
    return RepaintBoundary(
      child: MouseRegion(
        cursor: SystemMouseCursors.click,
        onEnter: (_) => setState(() => _hovered = true),
        onExit: (_) => setState(() => _hovered = false),
        child: GestureDetector(
          onTap: widget.onTap,
          onLongPress: () => _showActions(context),
          onSecondaryTap: () => _showActions(context),
          child: AnimatedContainer(
            duration: const Duration(milliseconds: 90),
            decoration: BoxDecoration(
              color: widget.selected ? ws.states.selected : ws.panel,
              borderRadius: BorderRadius.circular(ws.cardRadius),
              border: Border.all(
                color: widget.selected
                    ? ws.accent
                    : _hovered
                    ? ws.textDim.withValues(alpha: 0.35)
                    : ws.border,
              ),
            ),
            clipBehavior: Clip.antiAlias,
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: [
                SizedBox(
                  height: SaveCard.coverHeight,
                  child: _Cover(path: widget.thumbnailPath),
                ),
                Expanded(
                  child: Padding(
                    padding: EdgeInsets.all(ws.cardPadding),
                    child: _Body(save: save, host: host),
                  ),
                ),
              ],
            ),
          ),
        ),
      ),
    );
  }

  void _showActions(BuildContext context) {
    final ws = context.ws;
    final save = widget.save;
    final controller = widget.controller;
    showModalBottomSheet<void>(
      context: context,
      backgroundColor: ws.panel,
      builder: (sheet) => SafeArea(
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            ListTile(
              leading: Icon(Icons.open_in_new, color: ws.textDim),
              title: Text('Open in browser', style: TextStyle(color: ws.text)),
              onTap: () {
                Navigator.pop(sheet);
                openInBrowser(save.url);
              },
            ),
            ListTile(
              leading: Icon(Icons.edit_outlined, color: ws.textDim),
              title: Text('Edit', style: TextStyle(color: ws.text)),
              onTap: () {
                Navigator.pop(sheet);
                showEditSheet(context, save, controller);
              },
            ),
            ListTile(
              leading: Icon(
                save.favorite ? Icons.star : Icons.star_outline,
                color: ws.textDim,
              ),
              title: Text(
                save.favorite ? 'Unstar' : 'Star',
                style: TextStyle(color: ws.text),
              ),
              onTap: () {
                Navigator.pop(sheet);
                controller.toggleFavorite(save);
              },
            ),
            ListTile(
              leading: Icon(Icons.delete_outline, color: ws.danger),
              title: Text('Delete', style: TextStyle(color: ws.danger)),
              onTap: () {
                Navigator.pop(sheet);
                confirmDelete(context, save, controller);
              },
            ),
          ],
        ),
      ),
    );
  }
}

class _Body extends StatelessWidget {
  const _Body({required this.save, required this.host});
  final SaveView save;
  final String host;

  @override
  Widget build(BuildContext context) {
    final ws = context.ws;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      mainAxisSize: MainAxisSize.min,
      children: [
        Row(
          children: [
            if (!save.isRead) ...[
              Container(
                width: 7,
                height: 7,
                decoration: BoxDecoration(
                  color: ws.accent,
                  shape: BoxShape.circle,
                ),
              ),
              const SizedBox(width: 6),
            ],
            Flexible(
              child: Text(
                save.title.isEmpty ? host : save.title,
                maxLines: 1,
                overflow: TextOverflow.ellipsis,
                style: TextStyle(
                  color: ws.text,
                  fontSize: 13.5,
                  fontWeight: FontWeight.w600,
                ),
              ),
            ),
            const SizedBox(width: 6),
            _StatusChip(status: save.status),
            if (save.favorite) ...[
              const SizedBox(width: 4),
              Icon(Icons.star, size: 13, color: ws.health.changed),
            ],
          ],
        ),
        const SizedBox(height: 3),
        Text(
          host,
          maxLines: 1,
          overflow: TextOverflow.ellipsis,
          style: TextStyle(color: ws.textDim, fontSize: 12),
        ),
        if (save.description.isNotEmpty) ...[
          const SizedBox(height: 6),
          Flexible(
            child: Text(
              save.description,
              maxLines: 2,
              overflow: TextOverflow.ellipsis,
              style: TextStyle(color: ws.textDim, fontSize: 12.5, height: 1.35),
            ),
          ),
        ],
        if (save.tags.isNotEmpty) ...[
          const SizedBox(height: 8),
          SizedBox(
            height: 20,
            child: ListView(
              scrollDirection: Axis.horizontal,
              physics: const NeverScrollableScrollPhysics(),
              children: [
                for (final tag in save.tags.take(3))
                  Padding(
                    padding: const EdgeInsets.only(right: 5),
                    child: Container(
                      padding: const EdgeInsets.symmetric(
                        horizontal: 7,
                        vertical: 2,
                      ),
                      decoration: BoxDecoration(
                        color: ws.background,
                        borderRadius: BorderRadius.circular(ws.iconTileRadius),
                      ),
                      child: Text(
                        '#$tag',
                        style: TextStyle(color: ws.textDim, fontSize: 11),
                      ),
                    ),
                  ),
              ],
            ),
          ),
        ],
      ],
    );
  }
}

/// The cover image, decoded at display size.
class _Cover extends StatelessWidget {
  const _Cover({required this.path});
  final String? path;

  /// Cards are ~320 logical px wide; 2x covers retina without decoding a
  /// full-resolution screenshot into memory for a thumbnail.
  static const int _decodeWidth = 640;

  @override
  Widget build(BuildContext context) {
    final ws = context.ws;
    if (path == null) {
      return Container(
        color: ws.background,
        child: Icon(Icons.language, size: 26, color: ws.border),
      );
    }
    return Image.file(
      File(path!),
      fit: BoxFit.cover,
      // Without this the full image is decoded no matter how small it is
      // drawn, which is what makes image grids janky.
      cacheWidth: _decodeWidth,
      filterQuality: FilterQuality.medium,
      gaplessPlayback: true,
      errorBuilder: (_, _, _) => Container(
        color: ws.background,
        child: Icon(Icons.broken_image_outlined, size: 22, color: ws.border),
      ),
    );
  }
}

class _StatusChip extends StatelessWidget {
  const _StatusChip({required this.status});
  final String status;

  @override
  Widget build(BuildContext context) {
    final ws = context.ws;
    final color = ws.health.of(status);
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 6, vertical: 1),
      decoration: BoxDecoration(
        color: color.withValues(alpha: 0.15),
        borderRadius: BorderRadius.circular(99),
      ),
      child: Text(
        status,
        style: TextStyle(
          color: color,
          fontSize: 10.5,
          fontWeight: FontWeight.w600,
        ),
      ),
    );
  }
}
