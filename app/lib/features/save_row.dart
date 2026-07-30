// One saved page in the list.
//
// The component the whole app's polish rests on, and the one place the
// hover audit matters. The React app reveals affordances on `:hover` in 25
// places; none of those exist on a touchscreen. So the rule here is:
//
//   - Information is always visible.
//   - Actions are revealed on hover *only when a pointer exists*, and are
//     otherwise reachable by long-press.
//
// `context.layout.showsHover` is how that decision is made — never
// `Platform.isIOS`, which would be wrong for an iPad with a trackpad.

import 'package:flutter/material.dart';

import '../shell/layout_scope.dart';
import '../src/rust/api/vault_api.dart';
import '../theme/websave_theme.dart';

class SaveRow extends StatefulWidget {
  const SaveRow({
    super.key,
    required this.save,
    required this.selected,
    required this.onTap,
  });

  final SaveView save;
  final bool selected;
  final VoidCallback onTap;

  @override
  State<SaveRow> createState() => _SaveRowState();
}

class _SaveRowState extends State<SaveRow> {
  bool _hovered = false;

  @override
  Widget build(BuildContext context) {
    final ws = context.ws;
    final layout = context.layout;
    final save = widget.save;
    final showActions = layout.showsHover ? _hovered || widget.selected : false;

    final background = widget.selected
        ? ws.states.selected
        : _hovered && layout.showsHover
        ? ws.states.hover
        : Colors.transparent;

    return MouseRegion(
      cursor: SystemMouseCursors.click,
      onEnter: (_) => setState(() => _hovered = true),
      onExit: (_) => setState(() => _hovered = false),
      child: GestureDetector(
        onTap: widget.onTap,
        // The touch replacement for a hover-revealed row menu.
        onLongPress: layout.showsHover ? null : () => _showActions(context),
        child: AnimatedContainer(
          duration: const Duration(milliseconds: 90),
          constraints: BoxConstraints(minHeight: ws.rowHeight + 16),
          padding: EdgeInsets.symmetric(
            horizontal: ws.cardPadding * 0.7,
            vertical: 7,
          ),
          decoration: BoxDecoration(
            color: background,
            borderRadius: BorderRadius.circular(ws.controlRadius),
          ),
          child: Row(
            crossAxisAlignment: CrossAxisAlignment.center,
            children: [
              _StatusPip(status: save.status),
              SizedBox(width: ws.cardPadding * 0.7),
              _IconTile(save: save),
              SizedBox(width: ws.cardPadding * 0.8),
              Expanded(child: _Titles(save: save)),
              if (!save.isRead) ...[
                const SizedBox(width: 8),
                Container(
                  width: 7,
                  height: 7,
                  decoration: BoxDecoration(
                    color: ws.accent,
                    shape: BoxShape.circle,
                  ),
                ),
              ],
              if (save.favorite) ...[
                const SizedBox(width: 8),
                Icon(Icons.star, size: 14, color: ws.health.changed),
              ],
              // Only rendered where a pointer can reach them. On touch the
              // same actions live behind the long-press sheet, so nothing is
              // unreachable.
              if (showActions) ...[
                const SizedBox(width: 4),
                _RowAction(icon: Icons.open_in_new, onTap: () {}),
                _RowAction(icon: Icons.more_horiz, onTap: () {}),
              ],
            ],
          ),
        ),
      ),
    );
  }

  void _showActions(BuildContext context) {
    final ws = context.ws;
    showModalBottomSheet<void>(
      context: context,
      backgroundColor: ws.panel,
      shape: RoundedRectangleBorder(
        borderRadius: BorderRadius.vertical(
          top: Radius.circular(ws.cardRadius * 1.4),
        ),
      ),
      builder: (context) => SafeArea(
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            ListTile(
              leading: Icon(Icons.open_in_new, color: ws.textDim),
              title: Text('Open', style: TextStyle(color: ws.text)),
              onTap: () => Navigator.pop(context),
            ),
            ListTile(
              leading: Icon(Icons.star_outline, color: ws.textDim),
              title: Text('Star', style: TextStyle(color: ws.text)),
              onTap: () => Navigator.pop(context),
            ),
            ListTile(
              leading: Icon(Icons.delete_outline, color: ws.danger),
              title: Text('Delete', style: TextStyle(color: ws.danger)),
              onTap: () => Navigator.pop(context),
            ),
          ],
        ),
      ),
    );
  }
}

class _StatusPip extends StatelessWidget {
  const _StatusPip({required this.status});
  final String status;

  @override
  Widget build(BuildContext context) {
    return Tooltip(
      message: status,
      child: Container(
        width: 8,
        height: 8,
        decoration: BoxDecoration(
          // The whole reason LinkHealthColors exists: no literal here.
          color: context.ws.health.of(status),
          shape: BoxShape.circle,
        ),
      ),
    );
  }
}

/// The bordered favicon tile — 30px in the React app, a token now so touch
/// packs can grow it.
class _IconTile extends StatelessWidget {
  const _IconTile({required this.save});
  final SaveView save;

  @override
  Widget build(BuildContext context) {
    final ws = context.ws;
    return Container(
      width: ws.iconTileSize,
      height: ws.iconTileSize,
      decoration: BoxDecoration(
        color: ws.background,
        border: Border.all(color: ws.border),
        borderRadius: BorderRadius.circular(ws.iconTileRadius),
      ),
      clipBehavior: Clip.antiAlias,
      child: save.faviconUrl.isEmpty
          ? Icon(Icons.language, size: 15, color: ws.textDim)
          : Padding(
              padding: const EdgeInsets.all(5),
              child: Image.network(
                save.faviconUrl,
                fit: BoxFit.contain,
                errorBuilder: (_, _, _) =>
                    Icon(Icons.language, size: 15, color: ws.textDim),
              ),
            ),
    );
  }
}

class _Titles extends StatelessWidget {
  const _Titles({required this.save});
  final SaveView save;

  @override
  Widget build(BuildContext context) {
    final ws = context.ws;
    final host = Uri.tryParse(save.url)?.host ?? save.url;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      mainAxisSize: MainAxisSize.min,
      children: [
        Text(
          save.title.isEmpty ? save.url : save.title,
          maxLines: 1,
          overflow: TextOverflow.ellipsis,
          style: TextStyle(
            color: ws.text,
            fontSize: 13.5,
            fontWeight: FontWeight.w600,
          ),
        ),
        const SizedBox(height: 1),
        Row(
          children: [
            Flexible(
              child: Text(
                host,
                maxLines: 1,
                overflow: TextOverflow.ellipsis,
                style: TextStyle(color: ws.textDim, fontSize: 12),
              ),
            ),
            if (save.tags.isNotEmpty) ...[
              const SizedBox(width: 8),
              Flexible(
                child: Text(
                  save.tags.take(3).map((t) => '#$t').join(' '),
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                  style: TextStyle(color: ws.textDim, fontSize: 12),
                ),
              ),
            ],
          ],
        ),
      ],
    );
  }
}

class _RowAction extends StatelessWidget {
  const _RowAction({required this.icon, required this.onTap});
  final IconData icon;
  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) {
    final ws = context.ws;
    return SizedBox(
      width: ws.hitTarget,
      height: ws.hitTarget,
      child: IconButton(
        padding: EdgeInsets.zero,
        iconSize: 16,
        color: ws.textDim,
        icon: Icon(icon),
        onPressed: onTap,
      ),
    );
  }
}
