// The vault is written by another process.
//
// The browser extension posts to the Tauri app's capture server, which writes
// the vault this app reads, so a save made while this window is open lands on
// disk with nothing to announce it. Polling is how the list finds out.
//
// What matters is that it is *quiet*: a poll must not flash the loading state
// over a list that is usually about to be identical, and must not clear an
// error the user has not read yet.

import 'package:fake_async/fake_async.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:websave_app/features/saves_controller.dart';

class _CountingController extends SavesController {
  int loads = 0;
  final quietness = <bool>[];

  @override
  Future<void> load({bool quiet = false}) async {
    loads++;
    quietness.add(quiet);
  }
}

void main() {
  test('polling reloads on a timer, quietly', () {
    fakeAsync((async) {
      final controller = _CountingController()..startPolling();

      async.elapse(const Duration(seconds: 13));

      expect(controller.loads, 3, reason: 'one every four seconds');
      expect(
        controller.quietness,
        everyElement(isTrue),
        reason: 'a poll that entered the loading state would flash a spinner '
            'over the list every few seconds',
      );
      controller.stopPolling();
    });
  });

  test('stopPolling really stops', () {
    fakeAsync((async) {
      final controller = _CountingController()..startPolling();
      async.elapse(const Duration(seconds: 5));
      final before = controller.loads;

      controller.stopPolling();
      async.elapse(const Duration(minutes: 2));

      expect(
        controller.loads,
        before,
        reason: 'backgrounded, nobody is looking and the wakeups cost battery',
      );
    });
  });

  test('starting twice does not double the rate', () {
    fakeAsync((async) {
      final controller = _CountingController()
        ..startPolling()
        ..startPolling();

      async.elapse(const Duration(seconds: 9));

      expect(controller.loads, 2);
      controller.stopPolling();
    });
  });
}
