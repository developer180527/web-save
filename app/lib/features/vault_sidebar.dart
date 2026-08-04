// The sidebar: sections, link health, tags.
//
// A port of the React app's sidebar, which is quiet chrome — dim labels,
// right-aligned counts, an active row that tints rather than highlights.
// Every count comes from the vault rather than from counting the loaded list,
// so "Dead 1" is true even when the list is showing something else.

import 'package:flutter/material.dart';

import '../theme/websave_theme.dart';
import 'saves_controller.dart';

class VaultSidebar extends StatelessWidget {
  const VaultSidebar({
    super.key,
    required this.controller,
    required this.trailing,
    this.onOpenSettings,
    this.onShowSaves,
  });

  final SavesController controller;
  final Widget trailing;
  final VoidCallback? onOpenSettings;

  /// Bring the saves list back into view.
  ///
  /// The sidebar is always visible, including while Settings or Import is
  /// showing, so picking a filter has to navigate as well as filter —
  /// otherwise the tap updates the highlight and the counts and leaves the
  /// main pane on whatever it was, which reads as the click being ignored.
  final VoidCallback? onShowSaves;

  void _select(SaveFilter filter) {
    controller.setFilter(filter);
    onShowSaves?.call();
  }

  void _selectTag(String? tag) {
    controller.setTag(tag);
    onShowSaves?.call();
  }

  @override
  Widget build(BuildContext context) {
    final ws = context.ws;
    final stats = controller.stats;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Padding(
          padding: const EdgeInsets.fromLTRB(18, 14, 18, 10),
          child: Text(
            'WebSave',
            style: TextStyle(
              color: ws.text,
              fontSize: 16,
              fontWeight: FontWeight.w700,
            ),
          ),
        ),
        Expanded(
          child: ListView(
            padding: const EdgeInsets.only(bottom: 8),
            children: [
              _Row(
                icon: Icons.bookmark_outline,
                label: 'All saves',
                count: stats?.total.toInt(),
                active:
                    controller.filter == SaveFilter.all &&
                    controller.activeTag == null,
                onTap: () => _select(SaveFilter.all),
              ),
              _Row(
                icon: Icons.inbox_outlined,
                label: 'Inbox',
                count: stats?.unread.toInt(),
                active: controller.filter == SaveFilter.inbox,
                onTap: () => _select(SaveFilter.inbox),
              ),
              _Row(
                icon: Icons.star_outline,
                label: 'Favorites',
                count: stats?.favorites.toInt(),
                active: controller.filter == SaveFilter.favorites,
                onTap: () => _select(SaveFilter.favorites),
              ),
              const _SectionLabel('Link health'),
              _HealthRow(
                onSelected: onShowSaves,
                controller: controller,
                filter: SaveFilter.active,
                icon: Icons.check_circle_outline,
                label: 'Active',
                count: stats?.active.toInt(),
              ),
              _HealthRow(
                onSelected: onShowSaves,
                controller: controller,
                filter: SaveFilter.changed,
                icon: Icons.change_circle_outlined,
                label: 'Changed',
                count: stats?.changed.toInt(),
              ),
              _HealthRow(
                onSelected: onShowSaves,
                controller: controller,
                filter: SaveFilter.redirected,
                icon: Icons.turn_right,
                label: 'Redirected',
                count: stats?.redirected.toInt(),
              ),
              _HealthRow(
                onSelected: onShowSaves,
                controller: controller,
                filter: SaveFilter.unreachable,
                icon: Icons.cloud_off_outlined,
                label: 'Unreachable',
                count: stats?.unreachable.toInt(),
              ),
              _HealthRow(
                onSelected: onShowSaves,
                controller: controller,
                filter: SaveFilter.dead,
                icon: Icons.cancel_outlined,
                label: 'Dead',
                count: stats?.dead.toInt(),
              ),
              _HealthRow(
                onSelected: onShowSaves,
                controller: controller,
                filter: SaveFilter.unchecked,
                icon: Icons.help_outline,
                label: 'Unchecked',
                count: stats?.unchecked.toInt(),
              ),
              if (controller.tags.isNotEmpty) ...[
                const _SectionLabel('Tags'),
                for (final tag in controller.tags)
                  _Row(
                    icon: Icons.tag,
                    label: tag.name,
                    count: tag.count.toInt(),
                    active: controller.activeTag == tag.name,
                    onTap: () => _selectTag(
                      controller.activeTag == tag.name ? null : tag.name,
                    ),
                  ),
              ],
            ],
          ),
        ),
        Divider(color: ws.border, height: 1),
        Padding(
          padding: const EdgeInsets.all(10),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              trailing,
              const SizedBox(height: 8),
              Row(
                children: [
                  Expanded(
                    child: Text(
                      'Re-check all links',
                      style: TextStyle(color: ws.textDim, fontSize: 12),
                    ),
                  ),
                  IconButton(
                    icon: const Icon(Icons.tune, size: 16),
                    color: ws.textDim,
                    tooltip: 'Settings',
                    onPressed: onOpenSettings,
                  ),
                ],
              ),
            ],
          ),
        ),
      ],
    );
  }
}

/// A health row tints its icon with the same colour the status pips use, so
/// the sidebar and the list speak one visual language.
class _HealthRow extends StatelessWidget {
  const _HealthRow({
    required this.controller,
    required this.filter,
    required this.icon,
    required this.label,
    required this.count,
    this.onSelected,
  });

  final SavesController controller;
  final SaveFilter filter;
  final IconData icon;
  final String label;
  final int? count;
  final VoidCallback? onSelected;

  @override
  Widget build(BuildContext context) {
    return _Row(
      icon: icon,
      label: label,
      count: count,
      active: controller.filter == filter,
      iconColor: context.ws.health.of(filter.status ?? 'unchecked'),
      onTap: () {
        controller.setFilter(filter);
        onSelected?.call();
      },
    );
  }
}

class _SectionLabel extends StatelessWidget {
  const _SectionLabel(this.text);
  final String text;

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.fromLTRB(18, 16, 18, 6),
      child: Text(
        text.toUpperCase(),
        style: TextStyle(
          color: context.ws.textDim,
          fontSize: 10.5,
          fontWeight: FontWeight.w700,
          letterSpacing: 0.7,
        ),
      ),
    );
  }
}

class _Row extends StatefulWidget {
  const _Row({
    required this.icon,
    required this.label,
    required this.active,
    required this.onTap,
    this.count,
    this.iconColor,
  });

  final IconData icon;
  final String label;
  final bool active;
  final VoidCallback onTap;
  final int? count;
  final Color? iconColor;

  @override
  State<_Row> createState() => _RowState();
}

class _RowState extends State<_Row> {
  bool _hovered = false;

  @override
  Widget build(BuildContext context) {
    final ws = context.ws;
    final foreground = widget.active ? ws.text : ws.textDim;
    return Padding(
      padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 1),
      child: MouseRegion(
        cursor: SystemMouseCursors.click,
        onEnter: (_) => setState(() => _hovered = true),
        onExit: (_) => setState(() => _hovered = false),
        child: GestureDetector(
          onTap: widget.onTap,
          child: AnimatedContainer(
            duration: const Duration(milliseconds: 90),
            height: ws.rowHeight,
            padding: const EdgeInsets.symmetric(horizontal: 10),
            decoration: BoxDecoration(
              color: widget.active
                  ? ws.states.selected
                  : _hovered
                  ? ws.states.hover
                  : Colors.transparent,
              borderRadius: BorderRadius.circular(ws.controlRadius),
            ),
            child: Row(
              children: [
                Icon(
                  widget.icon,
                  size: 16,
                  color: widget.iconColor ?? foreground,
                ),
                const SizedBox(width: 10),
                Expanded(
                  child: Text(
                    widget.label,
                    maxLines: 1,
                    overflow: TextOverflow.ellipsis,
                    style: TextStyle(
                      color: foreground,
                      fontSize: 13,
                      fontWeight: widget.active
                          ? FontWeight.w600
                          : FontWeight.w500,
                    ),
                  ),
                ),
                if (widget.count != null)
                  Text(
                    '${widget.count}',
                    style: TextStyle(color: ws.textDim, fontSize: 11.5),
                  ),
              ],
            ),
          ),
        ),
      ),
    );
  }
}
