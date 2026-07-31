// The controller publishes its state as several notifiers rather than one.
//
// It used to be a single ChangeNotifier, so tapping a sidebar filter notified
// everything twice — once entering the loading state, once with results — and
// each notification repainted every visible card and thumbnail. The highlight
// was always set synchronously; it just had to arrive inside an expensive
// frame, which is what made it feel slow.
//
// These assert the boundaries hold: the narrowing is visible before any query
// runs, and the regions that should be independent do not notify each other.

import 'package:flutter/foundation.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:websave_app/features/saves_controller.dart';

/// The real load() calls the Rust bridge. Only that is stubbed — the criteria
/// bookkeeping under test is the real implementation.
class _OfflineController extends SavesController {
  int loads = 0;

  @override
  Future<void> load() async {
    loads++;
  }
}

void main() {
  test('the narrowing is published before the query runs', () {
    final controller = _OfflineController();
    var criteriaChanges = 0;
    controller.criteria.addListener(() => criteriaChanges++);

    // Not awaited: the point is that the sidebar can already paint the new
    // highlight on this frame, without the vault having answered.
    controller.setFilter(SaveFilter.inbox);

    expect(controller.criteria.value.filter, SaveFilter.inbox);
    expect(criteriaChanges, 1);
    expect(controller.loads, 1);
  });

  test('selecting a row does not notify the list', () {
    final controller = _OfflineController();
    var listNotifications = 0;
    controller.addListener(() => listNotifications++);

    controller.select(null);
    controller.gridView.value = false;

    expect(
      listNotifications,
      0,
      reason: 'selection and view mode are their own notifiers, so neither '
          'can drag the grid into a rebuild',
    );
  });

  test('a filter tap clears the tag, and a tag tap clears the filter', () {
    final controller = _OfflineController();

    controller.setTag('rust');
    expect(controller.criteria.value.tag, 'rust');
    expect(controller.criteria.value.filter, SaveFilter.all);

    controller.setFilter(SaveFilter.favorites);
    expect(
      controller.criteria.value.tag,
      isNull,
      reason: 'two narrowings with only one visible is how people end up '
          'staring at an empty list',
    );
  });

  test('the query survives a change of section', () {
    final controller = _OfflineController();

    controller.search('rust');
    controller.setFilter(SaveFilter.inbox);

    expect(controller.criteria.value.query, 'rust');
    expect(controller.criteria.value.filter, SaveFilter.inbox);
  });

  test('criteria compare by value, so an identical tap is not a change', () {
    const a = SaveCriteria(filter: SaveFilter.inbox, query: 'x');
    const b = SaveCriteria(filter: SaveFilter.inbox, query: 'x');
    expect(a, b);
    // ValueNotifier suppresses a set to an equal value, which is what stops a
    // repeated tap on the same row costing a rebuild.
    final notifier = ValueNotifier<SaveCriteria>(a);
    var changes = 0;
    notifier.addListener(() => changes++);
    notifier.value = b;
    expect(changes, 0);
  });
}
