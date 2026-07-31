// The shell-selection rule, which is easy to get subtly wrong and invisible
// when it is: a wrong answer here just looks like a slightly odd layout.

import 'package:flutter_test/flutter_test.dart';
import 'package:websave_app/shell/layout_scope.dart';

void main() {
  group('width alone', () {
    test('picks a shell by the two breakpoints', () {
      expect(LayoutInfo.sizeForWidth(1400), ShellSize.expanded);
      expect(LayoutInfo.sizeForWidth(kDesktopBreakpoint), ShellSize.expanded);
      expect(LayoutInfo.sizeForWidth(kDesktopBreakpoint - 1), ShellSize.medium);
      expect(LayoutInfo.sizeForWidth(kTabletBreakpoint), ShellSize.medium);
      expect(LayoutInfo.sizeForWidth(kTabletBreakpoint - 1), ShellSize.compact);
      expect(LayoutInfo.sizeForWidth(320), ShellSize.compact);
    });
  });

  group('platform floor', () {
    // A bottom tab bar is a thumb idiom. A desktop window dragged narrow
    // wants the icon rail instead -- the mouse reaches a rail perfectly well,
    // and a tab bar on a desktop just looks like a phone app pasted in.
    test('a desktop window never reaches the compact shell', () {
      for (final width in [560, 400, 320, 120]) {
        expect(
          LayoutInfo.sizeFor(width.toDouble(), touch: false),
          ShellSize.medium,
          reason: '$width px on desktop should floor at the rail',
        );
      }
    });

    test('touch platforms can use all three', () {
      expect(LayoutInfo.sizeFor(320, touch: true), ShellSize.compact);
      expect(LayoutInfo.sizeFor(800, touch: true), ShellSize.medium);
      expect(LayoutInfo.sizeFor(1200, touch: true), ShellSize.expanded);
    });

    // The iPad is the case that makes width-not-platform worth the trouble:
    // one device produces all three answers depending on how it is being
    // used, and none of them is "phone".
    test('an iPad spans the range as its window changes', () {
      expect(LayoutInfo.sizeFor(1024, touch: true), ShellSize.expanded);
      expect(LayoutInfo.sizeFor(694, touch: true), ShellSize.medium);
      expect(LayoutInfo.sizeFor(320, touch: true), ShellSize.compact);
    });

    test('the floor only ever raises, never lowers', () {
      for (final width in [320.0, 700.0, 1400.0]) {
        final touch = LayoutInfo.sizeFor(width, touch: true);
        final desktop = LayoutInfo.sizeFor(width, touch: false);
        expect(desktop.index, greaterThanOrEqualTo(touch.index));
      }
    });
  });

  group('derived flags', () {
    LayoutInfo info(ShellSize size, {bool touch = false}) => LayoutInfo(
      size: size,
      width: 0,
      isTouchPlatform: touch,
      hasPrecisedPointer: !touch,
    );

    test('only the expanded shell shows a sidebar and a detail pane', () {
      expect(info(ShellSize.expanded).showsSidebar, isTrue);
      expect(info(ShellSize.expanded).showsDetailPane, isTrue);
      expect(info(ShellSize.medium).showsSidebar, isFalse);
      expect(info(ShellSize.medium).showsDetailPane, isFalse);
    });

    test('the custom title bar is desktop-only', () {
      expect(info(ShellSize.expanded).showsCustomTitleBar, isTrue);
      // An iPad at desktop width still has no window chrome to replace.
      expect(
        info(ShellSize.expanded, touch: true).showsCustomTitleBar,
        isFalse,
      );
      expect(info(ShellSize.compact, touch: true).showsCustomTitleBar, isFalse);
    });

    test('hover affordances follow the pointer, not the size', () {
      expect(info(ShellSize.medium).showsHover, isTrue);
      expect(info(ShellSize.expanded, touch: true).showsHover, isFalse);
    });
  });
}