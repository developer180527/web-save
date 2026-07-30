// The seam between "what device is this" and "what should the theme be".
//
// Density is a theme token, but *which* density is right is a property of the
// pointer. Rather than let widgets ask Platform.isIOS — which would be wrong
// on an iPad with a trackpad, and wrong again on a touchscreen laptop — the
// question is answered once, here, and expressed as a pack choice.
//
// Layout is deliberately NOT decided here. Sidebar-vs-tabs follows window
// width (see shell/layout_scope.dart) so a narrow window on a Mac gets the
// compact layout without pretending to be a phone.

import 'package:flutter/material.dart';
import 'package:flutter_theme_engine/flutter_theme_engine.dart';

/// Flag keys this app puts into [ThemeContext.flags].
class ThemeFlags {
  /// `touch` or `mouse`.
  static const pointer = 'pointer';

  /// The pack id the user (or another trigger) settled on, so density can
  /// pick the matching variant rather than a hardcoded one.
  static const base = 'base';

  /// `main` or `quick` — the quick panel is a different surface.
  static const window = 'window';
}

/// Swaps the active pack for its roomier sibling when the pointer is coarse.
///
/// Priority sits below `UserSelectionTrigger` (1000) so an explicit choice
/// still wins, but above brightness (50) so a user on a phone who picked
/// "Midnight" gets `midnight_touch` rather than the desktop metrics.
class PointerDensityTrigger extends ThemeTrigger {
  const PointerDensityTrigger();

  @override
  int get priority => 800;

  @override
  String? resolve(ThemeContext ctx, Set<String> availableIds) {
    // flags is Map<String, Object?> — anything can be put in it, so read
    // defensively rather than casting.
    if (ctx.flags[ThemeFlags.pointer] != 'touch') return null;
    final base = ctx.flags[ThemeFlags.base];
    if (base is! String || base.endsWith('_touch')) return null;
    final touch = '${base}_touch';
    return availableIds.contains(touch) ? touch : null;
  }
}

/// Whether this device's primary pointer is a fingertip.
///
/// Read from the pointer devices Flutter actually sees rather than from the
/// platform, so an iPad with a Magic Keyboard is treated as a mouse and a
/// touchscreen Windows laptop is treated as touch.
bool isCoarsePointer(BuildContext context) {
  final data = MediaQuery.maybeOf(context);
  if (data == null) return false;
  // navigationMode + the absence of a precise pointer is the closest thing
  // Flutter exposes to CSS's `pointer: coarse`.
  return switch (Theme.of(context).platform) {
    TargetPlatform.iOS || TargetPlatform.android => true,
    _ => false,
  };
}
