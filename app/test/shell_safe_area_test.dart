// Every shell has to inset its *content*, not just its own chrome.
//
// This has now gone wrong three times in a row, each time the same way: the
// shell carefully insets the thing it draws itself — the nav bar, the rail,
// the sidebar — and hands the content pane the full window. On desktop that
// is invisible, because the custom title bar reserves the top edge. On a
// phone or an iPad the first line of every screen lands under the status bar
// clock.
//
// So this asserts on the content, from a fake viewport with real insets,
// rather than on the presence of a SafeArea widget somewhere in the tree.

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:flutter_theme_engine/flutter_theme_engine.dart';
import 'package:websave_app/shell/app_shell.dart';
import 'package:websave_app/shell/desktop_shell.dart';
import 'package:websave_app/shell/mobile_shell.dart';
import 'package:websave_app/shell/tablet_shell.dart';
import 'package:websave_app/theme/packs.dart';
import 'package:websave_app/theme/websave_theme.dart';

/// A status bar's worth of inset at the top, a home indicator at the bottom.
const _insets = EdgeInsets.only(top: 59, bottom: 34);

final _key = GlobalKey();

List<ShellDestination> _destinations() => [
  ShellDestination(
    id: 'content',
    label: 'Content',
    icon: Icons.circle,
    builder: (_) => SizedBox.expand(
      child: Align(
        alignment: Alignment.topLeft,
        child: SizedBox(key: _key, width: 10, height: 10),
      ),
    ),
  ),
];

Future<double> _contentTop(WidgetTester tester, Widget shell) async {
  final engine = ThemeEngine<WebSaveTheme>()..register(webSavePacks);
  await engine.restore();
  await tester.pumpWidget(
    ThemeEngineScope<WebSaveTheme>(
      engine: engine,
      child: MediaQuery(
        data: const MediaQueryData(padding: _insets),
        child: MaterialApp(home: shell),
      ),
    ),
  );
  return tester.getTopLeft(find.byKey(_key)).dy;
}

void main() {
  testWidgets('MobileShell keeps content clear of the status bar', (
    tester,
  ) async {
    final top = await _contentTop(
      tester,
      MobileShell(destinations: _destinations(), selected: 0, onSelect: (_) {}),
    );
    expect(
      top,
      greaterThanOrEqualTo(_insets.top),
      reason: 'content started at $top, under a ${_insets.top}pt status bar',
    );
  });

  testWidgets('TabletShell keeps content clear of the status bar', (
    tester,
  ) async {
    final top = await _contentTop(
      tester,
      TabletShell(destinations: _destinations(), selected: 0, onSelect: (_) {}),
    );
    expect(top, greaterThanOrEqualTo(_insets.top));
  });

  testWidgets('DesktopShell keeps content clear of the status bar', (
    tester,
  ) async {
    final top = await _contentTop(
      tester,
      DesktopShell(
        destinations: _destinations(),
        selected: 0,
        onSelect: (_) {},
        sidebar: const SizedBox.shrink(),
      ),
    );
    expect(top, greaterThanOrEqualTo(_insets.top));
  });
}
