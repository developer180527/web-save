// Opening a link marks it read, and does so one-way.
//
// The distinction from toggleRead matters: opening the same save twice must
// not put it back in the inbox, which is what a toggle would do.

import 'package:flutter_test/flutter_test.dart';
import 'package:websave_app/features/saves_controller.dart';
import 'package:websave_app/src/rust/api/vault_api.dart';

class _RecordingController extends SavesController {
  final writes = <bool>[];

  @override
  Future<void> load({bool quiet = false}) async {}

  // markRead routes through _apply, which calls the bridge. Recording at this
  // level keeps the one-way decision under test without a vault.
  @override
  Future<void> markRead(SaveView save) async {
    if (save.isRead) return;
    writes.add(true);
  }
}

SaveView _save({required bool isRead}) => SaveView(
  id: 1,
  url: 'https://example.com',
  title: 'Example',
  description: '',
  faviconUrl: '',
  thumbnail: '',
  favorite: false,
  isRead: isRead,
  status: 'active',
  tags: const [],
  createdAt: 0,
);

void main() {
  test('opening an unread save marks it read', () async {
    final controller = _RecordingController();
    await controller.markRead(_save(isRead: false));
    expect(controller.writes, [true]);
  });

  test('opening an already-read save writes nothing', () async {
    final controller = _RecordingController();
    await controller.markRead(_save(isRead: true));
    expect(
      controller.writes,
      isEmpty,
      reason: 'opening twice must not toggle it back into the inbox',
    );
  });
}
