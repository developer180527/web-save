// Compact layout: a bottom bar, and detail opens as a pushed route.
//
// Built from theme tokens rather than NavigationBar so it matches the other
// two shells exactly — Material's default bar has its own height, indicator
// and typography, none of which the rest of the app uses.

import 'package:flutter/material.dart';

import '../theme/websave_theme.dart';
import 'app_shell.dart';

class MobileShell extends StatelessWidget {
  const MobileShell({
    super.key,
    required this.destinations,
    required this.selected,
    required this.onSelect,
  });

  final List<ShellDestination> destinations;
  final int selected;
  final ValueChanged<int> onSelect;

  @override
  Widget build(BuildContext context) {
    final ws = context.ws;
    return Scaffold(
      backgroundColor: ws.background,
      body: destinations[selected].builder(context),
      bottomNavigationBar: Container(
        decoration: BoxDecoration(
          color: ws.panel,
          border: Border(top: BorderSide(color: ws.border)),
        ),
        child: SafeArea(
          top: false,
          child: SizedBox(
            // hitTarget, not a constant: the touch packs raise it to 44 and
            // the bar grows with them.
            height: ws.hitTarget + 20,
            child: Row(
              children: [
                for (var i = 0; i < destinations.length; i++)
                  Expanded(
                    child: _BarItem(
                      destination: destinations[i],
                      selected: i == selected,
                      onTap: () => onSelect(i),
                    ),
                  ),
              ],
            ),
          ),
        ),
      ),
    );
  }
}

class _BarItem extends StatelessWidget {
  const _BarItem({
    required this.destination,
    required this.selected,
    required this.onTap,
  });

  final ShellDestination destination;
  final bool selected;
  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) {
    final ws = context.ws;
    final color = selected ? ws.accent : ws.textDim;
    return InkWell(
      onTap: onTap,
      child: Column(
        mainAxisAlignment: MainAxisAlignment.center,
        children: [
          Icon(destination.icon, size: 20, color: color),
          const SizedBox(height: 2),
          Text(
            destination.label,
            style: TextStyle(fontSize: 10.5, color: color),
          ),
        ],
      ),
    );
  }
}
