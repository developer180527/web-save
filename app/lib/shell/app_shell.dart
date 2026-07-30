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
  const AppShell({super.key, required this.destinations, this.trailing});

  final List<ShellDestination> destinations;

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
        final info = LayoutInfo(
          width: constraints.maxWidth,
          size: LayoutInfo.sizeFor(constraints.maxWidth),
          // Hover exists when a precise pointer might; the platform is a
          // proxy here, and the trigger in theme/ makes the same call for
          // density. Both are conservative: assume touch on mobile.
          hasPrecisedPointer: switch (Theme.of(context).platform) {
            TargetPlatform.iOS || TargetPlatform.android => false,
            _ => true,
          },
        );

        final shell = switch (info.size) {
          ShellSize.expanded => DesktopShell(
            destinations: widget.destinations,
            selected: _index,
            onSelect: _select,
            trailing: widget.trailing,
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

        return LayoutScope(info: info, child: shell);
      },
    );
  }

  void _select(int index) => setState(() => _index = index);
}
