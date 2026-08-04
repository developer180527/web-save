// The sidebar is visible at the same time as Settings and Import, so a filter
// tap has to move the main pane as well as change the filter. Without that
// the tap updates the highlight and the counts and leaves the content pane
// alone, which reads as the click doing nothing.
//
// This pumps the real VaultSidebar rather than a stand-in, because the bug it
// guards was precisely that the widget changed state without telling the
// shell — a test over fakes would have passed with the bug still in.

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:flutter_theme_engine/flutter_theme_engine.dart';
import 'package:websave_app/features/saves_controller.dart';
import 'package:websave_app/features/vault_sidebar.dart';
import 'package:websave_app/theme/packs.dart';
import 'package:websave_app/theme/websave_theme.dart';

/// The real controller talks to the Rust bridge on every filter change, which
/// a widget test has no way to stand up. Only [load] is stubbed — the filter
/// bookkeeping under test is the real implementation.
class _OfflineController extends SavesController {
  @override
  Future<void> load({bool quiet = false}) async {}
}

Future<void> _pump(WidgetTester tester, VaultSidebar sidebar) async {
  final engine = ThemeEngine<WebSaveTheme>()..register(webSavePacks);
  await engine.restore();
  await tester.pumpWidget(
    ThemeEngineScope<WebSaveTheme>(
      engine: engine,
      child: MaterialApp(
        home: Scaffold(body: SizedBox(width: 260, child: sidebar)),
      ),
    ),
  );
}

void main() {
  testWidgets('picking a filter asks the shell for the saves list', (
    tester,
  ) async {
    final controller = _OfflineController();
    var shown = 0;

    await _pump(
      tester,
      VaultSidebar(
        controller: controller,
        trailing: const SizedBox.shrink(),
        onShowSaves: () => shown++,
      ),
    );

    await tester.tap(find.text('Inbox'));
    await tester.pump();

    expect(controller.filter, SaveFilter.inbox);
    expect(
      shown,
      1,
      reason:
          'the filter alone is not enough — the shell has to switch back '
          'to the saves destination or the main pane never changes',
    );
  });

  testWidgets('every top-level filter navigates, not just Inbox', (
    tester,
  ) async {
    final controller = _OfflineController();
    var shown = 0;

    await _pump(
      tester,
      VaultSidebar(
        controller: controller,
        trailing: const SizedBox.shrink(),
        onShowSaves: () => shown++,
      ),
    );

    for (final label in ['Favorites', 'All saves', 'Active']) {
      await tester.tap(find.text(label));
      await tester.pump();
    }

    expect(shown, 3, reason: 'health rows navigate the same as the top rows');
  });

  testWidgets('a sidebar with no shell attached still filters', (tester) async {
    // onShowSaves is optional: on a narrow layout the sidebar is a drawer over
    // the list, where there is nothing to navigate to.
    final controller = _OfflineController();

    await _pump(
      tester,
      VaultSidebar(controller: controller, trailing: const SizedBox.shrink()),
    );

    await tester.tap(find.text('Inbox'));
    await tester.pump();

    expect(controller.filter, SaveFilter.inbox);
  });
}
