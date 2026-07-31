// Picks a shell by width and hands the chosen one the same content.
//
// The three shells differ only in chrome: where navigation lives and whether
// a detail pane exists. None of them knows anything about saves, and the
// feature widgets they host know nothing about them in return.

import 'package:flutter/material.dart';

import 'desktop_shell.dart';
import 'layout_scope.dart';
import 'mobile_shell.dart';
import 'tablet_shell.dart';
import 'title_bar.dart';

/// A destination in the app's primary navigation.
@immutable
class ShellDestination {
  const ShellDestination({
    required this.id,
    required this.label,
    required this.icon,
    required this.builder,
  });

  final String id;
  final String label;
  final IconData icon;
  final WidgetBuilder builder;
}

class AppShell extends StatefulWidget {
  const AppShell({
    super.key,
    required this.destinations,
    this.trailing,
    this.sidebar,
  });

  final List<ShellDestination> destinations;

  /// Replaces the destination list in the expanded shell. The saves screen
  /// supplies the real sidebar — sections, link health, tags — which needs
  /// the controller and so cannot be built from destinations alone.
  final Widget Function(BuildContext, int, ValueChanged<int>)? sidebar;

  /// Extra chrome the desktop and tablet shells show in the sidebar footer —
  /// the theme picker, vault path, that sort of thing.
  final WidgetBuilder? trailing;

  @override
  State<AppShell> createState() => _AppShellState();
}

class _AppShellState extends State<AppShell> {
  int _index = 0;

  @override
  Widget build(BuildContext context) {
    return LayoutBuilder(
      builder: (context, constraints) {
        final touch = switch (Theme.of(context).platform) {
          TargetPlatform.iOS || TargetPlatform.android => true,
          _ => false,
        };
        final info = LayoutInfo(
          width: constraints.maxWidth,
          // Floored on desktop so a narrowed window never reaches the bottom
          // bar — see the note in layout_scope.dart.
          size: LayoutInfo.sizeFor(constraints.maxWidth, touch: touch),
          isTouchPlatform: touch,
          // Hover exists when a precise pointer might. Conservative: assume
          // fingers on a touch platform even though an iPad may have a
          // trackpad, because a missing affordance is worse than a spare one.
          hasPrecisedPointer: !touch,
        );

        final shell = switch (info.size) {
          ShellSize.expanded => DesktopShell(
            destinations: widget.destinations,
            selected: _index,
            onSelect: _select,
            trailing: widget.trailing,
            sidebar: widget.sidebar == null
                ? null
                : widget.sidebar!(context, _index, _select),
          ),
          ShellSize.medium => TabletShell(
            destinations: widget.destinations,
            selected: _index,
            onSelect: _select,
            trailing: widget.trailing,
          ),
          ShellSize.compact => MobileShell(
            destinations: widget.destinations,
            selected: _index,
            onSelect: _select,
          ),
        };

        // The title bar sits above every shell and renders nothing where
        // there is no window chrome to replace.
        return LayoutScope(
          info: info,
          child: Column(
            children: [
              const AppTitleBar(),
              Expanded(child: shell),
            ],
          ),
        );
      },
    );
  }

  void _select(int index) => setState(() => _index = index);
}
