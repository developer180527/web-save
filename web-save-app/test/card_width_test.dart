// Cards are a fixed width; only the column count reflows.
//
// They used to stretch to fill the pane, so dragging the window edge resized
// every visible card and relaid out every thumbnail. The Tauri app holds them
// still, which reads as steady rather than elastic, and this pins that.
//
// Asserted arithmetically rather than by pumping a grid: the property is a
// property of the layout maths, and a widget test would mostly be measuring
// GridView rather than the decision under test.

import 'package:flutter_test/flutter_test.dart';
import 'package:websave_app/features/save_card.dart';

/// The column calculation from SavesScreen, kept in step with it.
int columnsFor(double available, {double gap = 12}) =>
    ((available + gap) / (SaveCard.width + gap)).floor().clamp(1, 12);

void main() {
  test('card width is a constant, not a range', () {
    // If a minWidth/maxWidth band ever comes back, maxWidth stops equalling
    // the fixed width and this fails.
    expect(SaveCard.maxWidth, SaveCard.width);
  });

  test('widening the pane adds columns without widening cards', () {
    const gap = 12.0;
    var lastColumns = 0;
    for (var width = 200.0; width < 2000; width += 7) {
      final columns = columnsFor(width);
      final used = columns * SaveCard.width + (columns - 1) * gap;

      expect(
        used,
        lessThanOrEqualTo(width + SaveCard.width),
        reason: 'the grid never claims more than the pane plus its overflow '
            'allowance at $width',
      );
      expect(
        columns,
        greaterThanOrEqualTo(lastColumns),
        reason: 'column count only ever grows as the pane widens',
      );
      lastColumns = columns;
    }
  });

  test('a pane narrower than one card still gets a column', () {
    expect(columnsFor(80), 1, reason: 'the card overflows, the layout does not');
  });

  test('card height follows its width, so the cover ratio is held', () {
    // The cover's ratio is fixed rather than its pixel height — that is what
    // stopped BoxFit.cover re-cropping screenshots during a window drag.
    final height = SaveCard.heightFor(SaveCard.width);
    expect(height, SaveCard.width / SaveCard.coverAspect + SaveCard.bodyHeight);
  });
}
