// Launch smoke test: does the built app actually run?
//
// Unit and widget tests run in the Dart VM against fakes. They cannot notice
// that the Rust library failed to link into a Windows ARM64 build, that a
// plugin is missing on Linux, or that the vault cannot be created where a
// platform puts app data. This boots the real `main()` — the same startup
// path a user gets — and does the smallest thing that proves the whole stack
// works: add a link through the UI, see it listed, find it by search. The
// add goes Dart → Rust → SQLite and the search goes back through FTS5, so a
// pass means every layer loaded and talks to the next.
//
// Run it with a throwaway vault, which the test insists on:
//
//   flutter drive --profile \
//     --driver=test_driver/integration_test.dart \
//     --target=integration_test/smoke_test.dart \
//     --dart-define=WEBSAVE_VAULT=/tmp/websave-smoke/vault \
//     -d macos
//
// Profile rather than debug where the platform allows it, because profile is
// AOT-compiled like the release build. Release itself cannot be driven: it
// strips the service protocol the test runner talks over.

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:integration_test/integration_test.dart';

import 'package:websave_app/main.dart' as app;

/// The vault this run writes to, from `--dart-define=WEBSAVE_VAULT=...`.
const _vault = String.fromEnvironment('WEBSAVE_VAULT');

/// Unique per run, so a vault reused between local runs cannot satisfy the
/// "it appeared" check with a save from last time.
final _marker = 'smoke-${DateTime.now().microsecondsSinceEpoch}';

void main() {
  IntegrationTestWidgetsFlutterBinding.ensureInitialized();

  testWidgets('launches, saves a link, and finds it again', (tester) async {
    // Without the override the app opens the user's own vault — on desktop,
    // the Tauri app's vault if there is one. A smoke test must never write a
    // fake bookmark into real data, so refuse rather than fall back.
    expect(
      _vault,
      isNotEmpty,
      reason:
          'Pass --dart-define=WEBSAVE_VAULT=<throwaway path>; without it '
          'this would write into your real vault.',
    );

    // An integration test binding leaves text input on the platform's real
    // keyboard channel, where `enterText` goes nowhere on desktop: the field
    // stays empty and the dialog answers "Enter a link to save." Route it
    // through the test text input instead, which delivers edits the way a
    // keyboard would, onChanged included.
    tester.testTextInput.register();

    app.main();

    // Startup: engine, vault, theme. Done when the toolbar is up.
    final addButton = find.byTooltip('Add a link');
    await _waitFor(tester, addButton, 'the app to finish starting');

    // Add two links the way a user would: the one we will search for, and
    // an unrelated one that search has to leave out.
    final url = 'https://$_marker.example/launch';
    final other = 'https://control-${_marker.hashCode}.example/';
    await _addThroughDialog(tester, addButton, url);
    await _addThroughDialog(tester, addButton, other);

    // Both reached the vault and came back out into the list. Matched on the
    // host, which every layout shows, rather than on the title, which a new
    // save does not have yet.
    final listed = find.textContaining('$_marker.example');
    final control = find.textContaining('control-${_marker.hashCode}');
    await _waitFor(tester, listed, 'the new save to appear in the list');
    await _waitFor(tester, control, 'the second save to appear in the list');

    // Full-text search finds the one and drops the other. Requiring the
    // control to disappear is what makes this a test of search: a search box
    // that ignored its query would still show the first save.
    await tester.enterText(
      find.byWidgetPredicate(
        (w) =>
            w is TextField &&
            (w.decoration?.hintText?.startsWith('Search') ?? false),
      ),
      _marker,
    );
    await _waitFor(
      tester,
      control,
      'search to hide the unrelated save',
      gone: true,
    );
    expect(listed, findsWidgets, reason: 'search dropped the matching save');
  });
}

/// Open the add dialog, type [url], save, and wait for the dialog to close.
Future<void> _addThroughDialog(
  WidgetTester tester,
  Finder addButton,
  String url,
) async {
  await tester.tap(addButton);
  final dialog = find.byType(AlertDialog);
  await _waitFor(tester, dialog, 'the add dialog');
  await tester.enterText(
    find.descendant(of: dialog, matching: find.byType(TextField)),
    url,
  );
  await tester.tap(find.widgetWithText(FilledButton, 'Save'));
  // Closing is the success signal: on an error the dialog stays open and
  // shows the message, which the timeout report will include.
  await _waitFor(
    tester,
    dialog,
    'the add dialog to close after saving $url',
    gone: true,
  );
}

/// Pump until [finder] matches (or, with [gone], stops matching), failing with a useful message if the app
/// reports a startup failure or nothing appears in time.
///
/// `pumpAndSettle` would never return here: the app polls the vault every few
/// seconds, so there is always another frame scheduled.
Future<void> _waitFor(
  WidgetTester tester,
  Finder finder,
  String what, {
  bool gone = false,
  Duration timeout = const Duration(seconds: 60),
}) async {
  final deadline = DateTime.now().add(timeout);
  while (DateTime.now().isBefore(deadline)) {
    await tester.pump(const Duration(milliseconds: 200));
    if (finder.evaluate().isNotEmpty != gone) return;

    // The bootstrap screen shows the real error when startup throws. Surface
    // it, rather than timing out a minute later with "not found".
    final failure = find.byType(app.StartupFailure);
    if (failure.evaluate().isNotEmpty) {
      final widget = tester.widget<app.StartupFailure>(failure);
      fail('App startup failed: ${widget.message}\n${widget.stack}');
    }
  }
  // What was on screen instead. On a CI runner nobody can look at the
  // window, so this is the only record of what the app was showing.
  final visible = find
      .byType(Text)
      .evaluate()
      .map((e) => (e.widget as Text).data)
      .whereType<String>()
      .where((s) => s.trim().isNotEmpty)
      .toSet()
      .join(' | ');
  fail(
    'Timed out after ${timeout.inSeconds}s waiting for $what.\n'
    'On screen: $visible',
  );
}
