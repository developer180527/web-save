// What shape the window is, kept strictly separate from what it looks like.
//
// Width decides how much room there is. Platform decides which *idioms* are
// available. Those are two different questions, and an earlier version of
// this file conflated them by letting width alone pick the shell -- which
// put a bottom tab bar on a narrowed desktop window. A bottom bar is a touch
// idiom reachable by a thumb, not a narrow-window idiom; a small desktop
// window wants the icon rail, because the mouse can reach a rail perfectly
// well and a tab bar there just looks like a phone app.
//
// So: the shell is `min(what the width affords, what the platform offers)`.
// Desktop floors at [ShellSize.medium]. Touch platforms can use all three,
// which keeps the iPad correct -- full screen it gets the sidebar, in a
// narrow Stage Manager slice it gets the rail, and an iPhone gets the bar.
//
// Density and colour remain the theme's business (theme/websave_theme.dart);
// which panes exist is this file's. Features never ask which shell they are
// in -- they read the booleans here, which is why one SaveRow serves all
// three.

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
    required this.isTouchPlatform,
  });

  final ShellSize size;
  final double width;

  /// Set once at the root from [Theme.of].platform. Distinct from
  /// [hasPrecisedPointer]: an iPad with a trackpad is a touch *platform* with
  /// a precise pointer, and the two answers differ.
  final bool isTouchPlatform;

  /// Whether a mouse or trackpad is driving. Hover affordances only exist
  /// when this is true — see the note in shell/README.
  final bool hasPrecisedPointer;

  bool get isCompact => size == ShellSize.compact;

  /// A persistent sidebar, rather than a drawer or bottom bar.
  bool get showsSidebar => size == ShellSize.expanded;

  /// Room for list and detail at the same time. Below this, opening a save
  /// pushes a route instead.
  ///
  /// Keyed to the actual width rather than to [size], which is what lets a
  /// desktop window keep its sidebar at any width without also trying to
  /// split a 500-point window down the middle.
  bool get showsDetailPane => width >= kDesktopBreakpoint;

  /// Whether to render hover-revealed affordances at all. On touch they are
  /// invisible, so those controls have to be permanent or move to a
  /// long-press menu.
  bool get showsHover => hasPrecisedPointer;

  /// Whether the platform's window has its own title bar, or we draw one.
  ///
  /// Desktop only: mobile and iPad have no window chrome to replace, and
  /// drawing a fake one there would just steal a row of screen.
  bool get showsCustomTitleBar => !isTouchPlatform;

  /// What the width alone would afford.
  static ShellSize sizeForWidth(double width) {
    if (width >= kDesktopBreakpoint) return ShellSize.expanded;
    if (width >= kTabletBreakpoint) return ShellSize.medium;
    return ShellSize.compact;
  }

  /// What the width affords, floored by what the platform should offer.
  ///
  /// Touch platforms follow the width. A desktop window does not: it stays
  /// expanded however narrow it is dragged.
  ///
  /// Resizing a window is not the same act as picking up a smaller device.
  /// Someone dragging a desktop window narrower has not changed what they
  /// want the app to be, and having the sidebar collapse into a rail
  /// underneath their cursor — then reappear on the way back out — is the
  /// layout arguing with them. The Tauri app keeps it, and so does every
  /// desktop app with a source list.
  ///
  /// The detail pane still follows the real width, so a narrow window gives
  /// the list all the room that is left rather than halving it.
  static ShellSize sizeFor(double width, {required bool touch}) {
    if (!touch) return ShellSize.expanded;
    return sizeForWidth(width);
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
