// Wide layout: a persistent sidebar beside the content.
//
// The sidebar is painted from theme tokens rather than Material's
// NavigationRail defaults, because the React app's sidebar is quiet chrome —
// dim labels, a hairline border, an active row that tints rather than
// highlights — and NavigationRail is much louder than that.

import 'package:flutter/material.dart';

import '../theme/websave_theme.dart';
import 'app_shell.dart';

class DesktopShell extends StatelessWidget {
  const DesktopShell({
    super.key,
    required this.destinations,
    required this.selected,
    required this.onSelect,
    this.trailing,
  });

  final List<ShellDestination> destinations;
  final int selected;
  final ValueChanged<int> onSelect;
  final WidgetBuilder? trailing;

  static const double sidebarWidth = 232;

  @override
  Widget build(BuildContext context) {
    final ws = context.ws;
    return Scaffold(
      backgroundColor: ws.background,
      body: Row(
        children: [
          Container(
            width: sidebarWidth,
            decoration: BoxDecoration(
              color: ws.translucentChrome ? ws.panel.withValues(alpha: 0.7) : ws.panel,
              border: Border(right: BorderSide(color: ws.border)),
            ),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: [
                const SizedBox(height: 12),
                for (var i = 0; i < destinations.length; i++)
                  SidebarRow(
                    destination: destinations[i],
                    selected: i == selected,
                    onTap: () => onSelect(i),
                  ),
                const Spacer(),
                if (trailing != null)
                  Padding(
                    padding: const EdgeInsets.all(12),
                    child: trailing!(context),
                  ),
              ],
            ),
          ),
          Expanded(child: destinations[selected].builder(context)),
        ],
      ),
    );
  }
}

/// One sidebar entry. Shared with the tablet shell so the two can never drift.
class SidebarRow extends StatefulWidget {
  const SidebarRow({
    super.key,
    required this.destination,
    required this.selected,
    required this.onTap,
    this.compact = false,
  });

  final ShellDestination destination;
  final bool selected;
  final VoidCallback onTap;
  final bool compact;

  @override
  State<SidebarRow> createState() => _SidebarRowState();
}

class _SidebarRowState extends State<SidebarRow> {
  bool _hovered = false;

  @override
  Widget build(BuildContext context) {
    final ws = context.ws;
    final background = widget.selected
        ? ws.states.selected
        : _hovered
        ? ws.states.hover
        : Colors.transparent;
    final foreground = widget.selected ? ws.text : ws.textDim;

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
            padding: EdgeInsets.symmetric(horizontal: widget.compact ? 0 : 10),
            decoration: BoxDecoration(
              color: background,
              borderRadius: BorderRadius.circular(ws.controlRadius),
            ),
            child: Row(
              mainAxisAlignment: widget.compact
                  ? MainAxisAlignment.center
                  : MainAxisAlignment.start,
              children: [
                Icon(widget.destination.icon, size: 17, color: foreground),
                if (!widget.compact) ...[
                  const SizedBox(width: 10),
                  Text(
                    widget.destination.label,
                    style: TextStyle(
                      color: foreground,
                      fontSize: 13.5,
                      fontWeight: widget.selected ? FontWeight.w600 : FontWeight.w500,
                    ),
                  ),
                ],
              ],
            ),
          ),
        ),
      ),
    );
  }
}
