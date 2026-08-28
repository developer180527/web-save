// Medium layout: an icon-only rail. iPad in portrait, or a half-width Mac
// window — the same layout serves both, because it is chosen by width.

import 'package:flutter/material.dart';

import '../theme/websave_theme.dart';
import 'app_shell.dart';
import 'desktop_shell.dart';

class TabletShell extends StatelessWidget {
  const TabletShell({
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

  static const double railWidth = 60;

  @override
  Widget build(BuildContext context) {
    final ws = context.ws;
    return Scaffold(
      backgroundColor: ws.background,
      body: Row(
        children: [
          Container(
            width: railWidth,
            decoration: BoxDecoration(
              color: ws.panel,
              border: Border(right: BorderSide(color: ws.border)),
            ),
            child: SafeArea(
              right: false,
              child: Column(
                children: [
                  const SizedBox(height: 12),
                  for (var i = 0; i < destinations.length; i++)
                    Tooltip(
                      message: destinations[i].label,
                      child: SidebarRow(
                        destination: destinations[i],
                        selected: i == selected,
                        onTap: () => onSelect(i),
                        compact: true,
                      ),
                    ),
                  const Spacer(),
                  if (trailing != null)
                    Padding(
                      padding: const EdgeInsets.only(bottom: 8),
                      child: trailing!(context),
                    ),
                ],
              ),
            ),
          ),
          // The rail insets itself; the content pane has to do the same or it
          // runs under the status bar. Left is already covered by the rail,
          // and the bottom is left to the destination to spend as it likes.
          Expanded(
            child: SafeArea(
              left: false,
              bottom: false,
              child: destinations[selected].builder(context),
            ),
          ),
        ],
      ),
    );
  }
}
