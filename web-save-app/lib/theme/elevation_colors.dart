// The lighting story: one fixed light, above and slightly in front.
//
// Everything here follows from that single premise. A raised surface catches
// light along its top edge and drops a shadow below it; a recessed one is the
// same gradient inverted, dark along its top lip and open at the bottom. Get
// the direction consistent and the eye assembles the depth for free — get one
// element inverted and the whole illusion reads as a mistake, which is the
// usual reason these attempts look wrong rather than merely subtle.
//
// Nothing here transforms, rotates, or takes perspective. Depth in a flat UI
// is a question of value and light, and faking a camera is what makes an
// interface look like a toy.
//
// Light and dark need genuinely different recipes, which is why these are per
// pack rather than derived. On a near-black ground a drop shadow is close to
// invisible — black at 30% over #161616 moves it barely two steps — so dark
// themes have to spend their depth budget on *value*: the raised surface sits
// a step lighter than the well and its top edge catches more light. Light
// themes are the opposite, and let the shadow do nearly all the work.

import 'dart:ui' show lerpDouble;

import 'package:flutter/material.dart';

@immutable
class ElevationColors {
  const ElevationColors({
    required this.well,
    required this.wellShade,
    required this.raisedEdge,
    required this.contact,
    required this.ambient,
    this.contactBlur = 2,
    this.ambientBlur = 8,
    this.ambientOffset = 2,
    this.lipDepth = 24,
  });

  /// The recessed surface content is kept on.
  ///
  /// Darker than the chrome around it, because a sunken surface catches less
  /// of the light.
  final Color well;

  /// The inner shadow along the well's top and leading lip.
  ///
  /// This is the single most load-bearing value in the file. It is what says
  /// the chrome is an edge and the content is *behind* it, rather than merely
  /// a different colour next to it.
  final Color wellShade;

  /// A hairline along the top edge of a raised surface.
  ///
  /// Carries most of the elevation in dark themes and almost none in light
  /// ones, where a highlight on a white card has nothing to lift against —
  /// hence fully transparent in Daylight rather than a token gesture.
  final Color raisedEdge;

  /// The tight, barely-offset shadow where an object meets the surface.
  ///
  /// Ambient occlusion, in effect. Without it a card reads as printed on the
  /// background; with it, as resting on it. A single shadow always looks like
  /// a sticker — the contact line is what makes it an object.
  final Color contact;

  /// The wider, softer shadow that carries actual height.
  final Color ambient;

  /// Blur radii, kept small on purpose. Blur cost scales with radius and
  /// these are painted once per card across a scrolling grid of hundreds.
  final double contactBlur;
  final double ambientBlur;
  final double ambientOffset;

  /// How far the well's inner shadow reaches before fading out.
  final double lipDepth;

  /// The two-layer stack for anything resting in the well.
  List<BoxShadow> get raised => [
    BoxShadow(
      color: contact,
      blurRadius: contactBlur,
      offset: const Offset(0, 1),
    ),
    BoxShadow(
      color: ambient,
      blurRadius: ambientBlur,
      offset: Offset(0, ambientOffset),
    ),
  ];

  static ElevationColors lerp(ElevationColors a, ElevationColors b, double t) =>
      ElevationColors(
        well: Color.lerp(a.well, b.well, t)!,
        wellShade: Color.lerp(a.wellShade, b.wellShade, t)!,
        raisedEdge: Color.lerp(a.raisedEdge, b.raisedEdge, t)!,
        contact: Color.lerp(a.contact, b.contact, t)!,
        ambient: Color.lerp(a.ambient, b.ambient, t)!,
        contactBlur: lerpDouble(a.contactBlur, b.contactBlur, t)!,
        ambientBlur: lerpDouble(a.ambientBlur, b.ambientBlur, t)!,
        ambientOffset: lerpDouble(a.ambientOffset, b.ambientOffset, t)!,
        lipDepth: lerpDouble(a.lipDepth, b.lipDepth, t)!,
      );

  @override
  bool operator ==(Object other) =>
      other is ElevationColors &&
      other.well == well &&
      other.wellShade == wellShade &&
      other.raisedEdge == raisedEdge &&
      other.contact == contact &&
      other.ambient == ambient &&
      other.contactBlur == contactBlur &&
      other.ambientBlur == ambientBlur &&
      other.ambientOffset == ambientOffset &&
      other.lipDepth == lipDepth;

  @override
  int get hashCode => Object.hash(
    well,
    wellShade,
    raisedEdge,
    contact,
    ambient,
    contactBlur,
    ambientBlur,
    ambientOffset,
    lipDepth,
  );
}
