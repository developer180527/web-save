import 'package:flutter/material.dart';

import '../src/rust/api/vault_api.dart';

/// The five states a saved link can be in.
///
/// This is WebSave's actual semantic vocabulary and Material has no word for
/// any of it — `error` is the closest, and it only covers one of the five. So
/// it lives on the theme model, which means a pack author restates five named
/// colours instead of guessing which greys were meant.
@immutable
class LinkHealthColors {
  const LinkHealthColors({
    required this.active,
    required this.changed,
    required this.redirected,
    required this.dead,
    required this.unchecked,
  });

  /// Reachable and unchanged.
  final Color active;

  /// Reachable, but the content hash moved.
  final Color changed;

  /// Now resolves somewhere meaningfully different.
  final Color redirected;

  /// Unreachable, or gone.
  final Color dead;

  /// Never checked.
  final Color unchecked;

  /// The colour for a status string as core reports it.
  ///
  /// Takes the raw string rather than an enum because that is what crosses
  /// the bridge; an unknown value falls back to `unchecked` instead of
  /// throwing, so a future status added in Rust degrades quietly.
  Color of(String status) => switch (status) {
    'active' => active,
    'changed' => changed,
    'redirected' => redirected,
    'dead' => dead,
    _ => unchecked,
  };

  Color ofSave(SaveView save) => of(save.status);

  LinkHealthColors copyWith({
    Color? active,
    Color? changed,
    Color? redirected,
    Color? dead,
    Color? unchecked,
  }) => LinkHealthColors(
    active: active ?? this.active,
    changed: changed ?? this.changed,
    redirected: redirected ?? this.redirected,
    dead: dead ?? this.dead,
    unchecked: unchecked ?? this.unchecked,
  );

  static LinkHealthColors lerp(
    LinkHealthColors a,
    LinkHealthColors b,
    double t,
  ) => LinkHealthColors(
    active: Color.lerp(a.active, b.active, t)!,
    changed: Color.lerp(a.changed, b.changed, t)!,
    redirected: Color.lerp(a.redirected, b.redirected, t)!,
    dead: Color.lerp(a.dead, b.dead, t)!,
    unchecked: Color.lerp(a.unchecked, b.unchecked, t)!,
  );

  Map<String, dynamic> toJson() => {
    'active': active.toARGB32(),
    'changed': changed.toARGB32(),
    'redirected': redirected.toARGB32(),
    'dead': dead.toARGB32(),
    'unchecked': unchecked.toARGB32(),
  };

  factory LinkHealthColors.fromJson(Map<String, dynamic> json) =>
      LinkHealthColors(
        active: Color(json['active'] as int),
        changed: Color(json['changed'] as int),
        redirected: Color(json['redirected'] as int),
        dead: Color(json['dead'] as int),
        unchecked: Color(json['unchecked'] as int),
      );
}

/// Surfaces for pointer and selection states.
///
/// `--bg-hover` and `--bg-active` in the CSS. Material expresses these as
/// overlay opacities on a base colour, which is a different model and gives
/// visibly different results — carrying them as explicit colours keeps the
/// port faithful and lets a pack pick them deliberately.
@immutable
class InteractionColors {
  const InteractionColors({
    required this.hover,
    required this.pressed,
    required this.selected,
  });

  final Color hover;
  final Color pressed;
  final Color selected;

  InteractionColors copyWith({Color? hover, Color? pressed, Color? selected}) =>
      InteractionColors(
        hover: hover ?? this.hover,
        pressed: pressed ?? this.pressed,
        selected: selected ?? this.selected,
      );

  static InteractionColors lerp(
    InteractionColors a,
    InteractionColors b,
    double t,
  ) => InteractionColors(
    hover: Color.lerp(a.hover, b.hover, t)!,
    pressed: Color.lerp(a.pressed, b.pressed, t)!,
    selected: Color.lerp(a.selected, b.selected, t)!,
  );

  Map<String, dynamic> toJson() => {
    'hover': hover.toARGB32(),
    'pressed': pressed.toARGB32(),
    'selected': selected.toARGB32(),
  };

  factory InteractionColors.fromJson(Map<String, dynamic> json) =>
      InteractionColors(
        hover: Color(json['hover'] as int),
        pressed: Color(json['pressed'] as int),
        selected: Color(json['selected'] as int),
      );
}
