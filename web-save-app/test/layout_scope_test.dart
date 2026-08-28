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
    test('a desktop window stays expanded however narrow it is dragged', () {
      // Resizing a window is not the same act as picking up a smaller device.
      // The sidebar collapsing under the cursor mid-drag, then reappearing on
      // the way back out, is the layout arguing with the user.
      for (final width in [1200, 900, 560, 400, 320, 120]) {
        expect(
          LayoutInfo.sizeFor(width.toDouble(), touch: false),
          ShellSize.expanded,
          reason: '$width px on desktop should keep the sidebar',
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
    // Width matters now: showsDetailPane is keyed to it rather than to size,
    // so a desktop window can keep its sidebar at any width without also
    // splitting itself in two.
    LayoutInfo info(
      ShellSize size, {
      bool touch = false,
      double width = 1200,
    }) => LayoutInfo(
      size: size,
      width: width,
      isTouchPlatform: touch,
      hasPrecisedPointer: !touch,
    );

    test('only the expanded shell shows a sidebar', () {
      expect(info(ShellSize.expanded).showsSidebar, isTrue);
      expect(info(ShellSize.medium).showsSidebar, isFalse);
    });

    test('the detail pane follows the real width, not the shell size', () {
      // The case that matters: a desktop window dragged narrow stays expanded
      // and keeps its sidebar, but must not try to show list and detail side
      // by side in the width that is left.
      expect(info(ShellSize.expanded, width: 1200).showsDetailPane, isTrue);
      expect(
        info(ShellSize.expanded, width: 700).showsDetailPane,
        isFalse,
        reason:
            'a narrow desktop window gives the list what room remains '
            'rather than halving it',
      );
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
