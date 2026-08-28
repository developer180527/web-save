// One light, from above. Every depth cue in the app follows from that.
//
// These guard the direction rather than the values: taste can move the alphas
// freely, but a shadow that lifts upward or a well lighter than the chrome it
// sits in inverts the lighting, and that is what makes the whole effect read
// as a mistake rather than as merely subtle.

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:websave_app/theme/packs.dart';

double _luminance(Color c) => c.computeLuminance();

void main() {
  for (final (name, theme) in [('daylight', daylight), ('midnight', midnight)]) {
    group(name, () {
      test('the well is darker than the chrome around it', () {
        expect(
          _luminance(theme.elevation.well),
          lessThan(_luminance(theme.panel)),
          reason: 'a recessed surface catches less light than the lip above '
              'it; inverting this reads as the content floating',
        );
      });

      test('cards sit lighter than the well they rest in', () {
        expect(
          _luminance(theme.panel),
          greaterThan(_luminance(theme.elevation.well)),
        );
      });

      test('every shadow falls downward', () {
        for (final shadow in theme.elevation.raised) {
          expect(
            shadow.offset.dy,
            greaterThanOrEqualTo(0),
            reason: 'one light source, above — an upward shadow anywhere '
                'collapses the illusion everywhere',
          );
          expect(shadow.offset.dx, 0, reason: 'the light is straight on');
        }
      });

      test('the contact shadow is tighter and darker than the ambient one', () {
        final [contact, ambient] = theme.elevation.raised;
        expect(
          contact.blurRadius,
          lessThan(ambient.blurRadius),
          reason: 'contact is occlusion at the meeting point, not height',
        );
        expect(contact.color.a, greaterThan(ambient.color.a));
      });

      test('blur stays cheap enough for a grid of hundreds', () {
        for (final shadow in theme.elevation.raised) {
          expect(shadow.blurRadius, lessThanOrEqualTo(12));
        }
      });
    });
  }

  test('dark leans on value, light leans on shadow', () {
    // The recipes genuinely differ. On a near-black ground a drop shadow is
    // nearly invisible, so Midnight spends its budget on the lit top edge;
    // Daylight has nothing to lift a highlight against and spends it on the
    // shadow instead.
    expect(
      midnight.elevation.raisedEdge.a,
      greaterThan(daylight.elevation.raisedEdge.a),
    );
    expect(daylight.elevation.raisedEdge.a, 0, reason: 'transparent, not token');
  });
}
