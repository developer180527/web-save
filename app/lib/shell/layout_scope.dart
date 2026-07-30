// What shape the window is, kept strictly separate from what it looks like.
//
// Layout follows *width*, never platform. A Mac window dragged narrow should
// become the compact layout, and an iPad in Stage Manager is genuinely
// somewhere between a phone and a desktop -- branching on Platform.isIOS gets
// both of those wrong. Density and colour are the theme's business
// (theme/websave_theme.dart); which panes exist is this file's.
//
// Features never ask which shell they are in. They read the two or three
// booleans here and lay themselves out accordingly, which is why one SaveCard
// serves all three shells.

import 'package:flutter/material.dart';

/// Width at which a persistent sidebar starts to fit.
const double kTabletBreakpoint = 640;

/// Width at which the list and a detail pane fit side by side.
const double kDesktopBreakpoint = 1000;

enum ShellSize { compact, medium, expanded }

@immutable
class LayoutInfo {
  const LayoutInfo({
    required this.size,
    required this.hasPrecisedPointer,
    required this.width,
  });

  final ShellSize size;
  final double width;

  /// Whether a mouse or trackpad is driving. Hover affordances only exist
  /// when this is true — see the note in shell/README.
  final bool hasPrecisedPointer;

  bool get isCompact => size == ShellSize.compact;

  /// A persistent sidebar, rather than a drawer or bottom bar.
  bool get showsSidebar => size == ShellSize.expanded;

  /// Room for list and detail at the same time. Below this, opening a save
  /// pushes a route instead.
  bool get showsDetailPane => size == ShellSize.expanded;

  /// Whether to render hover-revealed affordances at all. On touch they are
  /// invisible, so those controls have to be permanent or move to a
  /// long-press menu.
  bool get showsHover => hasPrecisedPointer;

  static ShellSize sizeFor(double width) {
    if (width >= kDesktopBreakpoint) return ShellSize.expanded;
    if (width >= kTabletBreakpoint) return ShellSize.medium;
    return ShellSize.compact;
  }
}

/// Provides [LayoutInfo] to the subtree.
class LayoutScope extends InheritedWidget {
  const LayoutScope({super.key, required this.info, required super.child});

  final LayoutInfo info;

  static LayoutInfo of(BuildContext context) {
    final scope = context.dependOnInheritedWidgetOfExactType<LayoutScope>();
    assert(scope != null, 'No LayoutScope above this widget');
    return scope!.info;
  }

  @override
  bool updateShouldNotify(LayoutScope oldWidget) => oldWidget.info != info;
}

extension LayoutContext on BuildContext {
  LayoutInfo get layout => LayoutScope.of(this);
}
