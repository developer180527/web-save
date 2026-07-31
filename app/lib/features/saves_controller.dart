// The list, the selection, and every write that touches them.
//
// Until now the list was a Future rebuilt by setState, which is fine for
// reading and wrong the moment anything writes: the list, the detail pane and
// the stats strip all show the same save, and a star toggled in one has to
// appear in the others immediately.
//
// A plain ChangeNotifier rather than a state-management package. The whole
// app's mutable state is a list, a selection and a couple of flags, and core
// already owns everything that matters.
//
// The rule that keeps this honest: every mutation returns the save as Rust
// now holds it, and that value replaces the row. Nothing is guessed locally,
// so the UI cannot drift from the vault -- and a 500-row list never refetches
// because one star changed.

import 'package:flutter/foundation.dart';

import '../src/rust/api/vault_api.dart';

/// What the sidebar is currently narrowed to. Mirrors the React app's
/// sections: everything, the Inbox, favourites, or one link-health state.
enum SaveFilter {
  all,
  inbox,
  favorites,
  active,
  changed,
  redirected,
  dead,
  unchecked,
}

extension SaveFilterQuery on SaveFilter {
  /// The `status` value core understands, or null when this filter is not
  /// about link health.
  String? get status => switch (this) {
    SaveFilter.active => 'active',
    SaveFilter.changed => 'changed',
    SaveFilter.redirected => 'redirected',
    SaveFilter.dead => 'dead',
    SaveFilter.unchecked => 'unchecked',
    _ => null,
  };

  bool get favoritesOnly => this == SaveFilter.favorites;
  bool get unreadOnly => this == SaveFilter.inbox;
}

/// What the list is currently narrowed to.
///
/// Grouped into one value because the three move together: picking a section
/// clears the tag, picking a tag resets the section, and both keep the query.
@immutable
class SaveCriteria {
  const SaveCriteria({this.filter = SaveFilter.all, this.tag, this.query = ''});

  final SaveFilter filter;
  final String? tag;
  final String query;

  @override
  bool operator ==(Object other) =>
      other is SaveCriteria &&
      other.filter == filter &&
      other.tag == tag &&
      other.query == query;

  @override
  int get hashCode => Object.hash(filter, tag, query);
}

class SavesController extends ChangeNotifier {
  /// The narrowing, published the instant a row is tapped and before any
  /// query runs.
  ///
  /// Split out so the sidebar can listen to this alone. It used to rebuild
  /// from the same notification as the grid, which meant a tap on a filter
  /// repainted every visible card — twice, once for the loading state and
  /// again for the results — before the highlight could move. The highlight
  /// was always set synchronously; it just arrived inside an expensive frame.
  final criteria = ValueNotifier<SaveCriteria>(const SaveCriteria());

  /// The sidebar's counts. Changes only when the vault does, not when the
  /// narrowing does, so it does not drag the sidebar into list rebuilds.
  final statsView = ValueNotifier<StatsView?>(null);

  /// The sidebar's tag list, on the same reasoning as [stats].
  final tagCounts = ValueNotifier<List<TagCountView>>(const []);

  /// The detail pane's subject. Selecting a row must not rebuild the grid.
  final selection = ValueNotifier<SaveView?>(null);

  /// Grid versus list. Read by the toggle and the results area only.
  final gridView = ValueNotifier<bool>(true);

  List<SaveView> _saves = const [];
  bool _loading = true;
  String? _error;

  /// Ids with a check in flight, so a row can show a spinner without the
  /// whole list going into a loading state.
  final Set<int> _checking = <int>{};

  /// Absolute path of the vault's assets directory. Thumbnails are stored as
  /// paths relative to it, so the grid needs it to build a File.
  String? _thumbnailRoot;
  String? thumbnailPath(SaveView save) =>
      (save.thumbnail.isEmpty || _thumbnailRoot == null)
      ? null
      : '$_thumbnailRoot/${save.thumbnail}';

  List<SaveView> get saves => _saves;
  StatsView? get stats => statsView.value;
  List<TagCountView> get tags => tagCounts.value;
  SaveFilter get filter => criteria.value.filter;
  String? get activeTag => criteria.value.tag;
  bool get isGridView => gridView.value;
  SaveView? get selected => selection.value;
  String get query => criteria.value.query;
  bool get isLoading => _loading;
  String? get error => _error;
  bool isChecking(int id) => _checking.contains(id);

  Future<void> load() async {
    final current = criteria.value;
    _loading = true;
    _error = null;
    notifyListeners();
    try {
      final results = await Future.wait([
        listSaves(
          query: current.query.isEmpty ? null : current.query,
          tag: current.tag,
          status: current.filter.status,
          favoritesOnly: current.filter.favoritesOnly,
          unreadOnly: current.filter.unreadOnly,
          limit: 2000,
        ),
        vaultStats(),
        listTags(),
      ]);
      // Taps outrun the vault: three rows in quick succession start three
      // loads, and they can finish out of order. Dropping a result whose
      // criteria are no longer current stops an older query overwriting a
      // newer one and leaving the list disagreeing with the sidebar.
      if (criteria.value != current) return;
      _saves = results[0] as List<SaveView>;
      statsView.value = results[1] as StatsView;
      tagCounts.value = results[2] as List<TagCountView>;
      _thumbnailRoot ??= await assetsDir();
      // A save can fall out of the result set when the query changes; a
      // selection pointing at a row that is no longer listed would leave the
      // detail pane showing something the list cannot highlight.
      final chosen = selection.value;
      if (chosen != null && !_saves.any((s) => s.id == chosen.id)) {
        selection.value = null;
      }
    } catch (e) {
      _error = '$e';
    } finally {
      _loading = false;
      notifyListeners();
    }
  }

  Future<void> search(String query) {
    criteria.value = SaveCriteria(
      filter: criteria.value.filter,
      tag: criteria.value.tag,
      query: query,
    );
    return load();
  }

  /// Picking a section clears any tag narrowing, and vice versa — two
  /// simultaneous narrowings with only one of them visible in the sidebar is
  /// how people end up staring at an empty list.
  Future<void> setFilter(SaveFilter filter) {
    criteria.value = SaveCriteria(filter: filter, query: criteria.value.query);
    return load();
  }

  Future<void> setTag(String? tag) {
    criteria.value = SaveCriteria(tag: tag, query: criteria.value.query);
    return load();
  }

  void toggleView() => gridView.value = !gridView.value;

  void select(SaveView? save) => selection.value = save;

  // ---- writes ----

  Future<void> toggleFavorite(SaveView save) =>
      _apply(() => setFavorite(id: save.id, favorite: !save.favorite));

  Future<void> toggleRead(SaveView save) =>
      _apply(() => setRead(id: save.id, isRead: !save.isRead));

  /// Mark read, if it is not already.
  ///
  /// Separate from [toggleRead] because opening a link is not a toggle: doing
  /// it twice should not put the save back in the inbox.
  Future<void> markRead(SaveView save) async {
    if (save.isRead) return;
    await _apply(() => setRead(id: save.id, isRead: true));
  }

  /// Patch the user-editable fields. `null` leaves a field untouched, which
  /// is core's own convention — an unchanged field must never overwrite what
  /// another writer put there.
  Future<void> edit(
    SaveView save, {
    String? title,
    String? description,
    String? notes,
  }) => _apply(
    () => updateSave(
      id: save.id,
      title: title,
      description: description,
      notes: notes,
    ),
  );

  Future<void> setTagsFor(SaveView save, List<String> tags) =>
      _apply(() => setTags(id: save.id, tags: tags));

  /// Re-check one link now. Slow (a real HTTP request), so it tracks a
  /// per-row pending state rather than blocking the list.
  Future<void> checkNow(SaveView save) async {
    if (_checking.contains(save.id)) return;
    _checking.add(save.id);
    notifyListeners();
    try {
      _replace(await checkSaveNow(id: save.id));
      // A check can change status, so the counts move with it.
      statsView.value = await vaultStats();
    } catch (e) {
      _error = '$e';
    } finally {
      _checking.remove(save.id);
      notifyListeners();
    }
  }

  Future<void> remove(SaveView save) async {
    try {
      await deleteSave(id: save.id);
      _saves = _saves.where((s) => s.id != save.id).toList();
      if (selection.value?.id == save.id) selection.value = null;
      statsView.value = await vaultStats();
    } catch (e) {
      _error = '$e';
    }
    notifyListeners();
  }

  /// Run a write and fold its result back into the list.
  Future<void> _apply(Future<SaveView> Function() write) async {
    try {
      _replace(await write());
      // Favourite and read counts live in stats, so they move too.
      statsView.value = await vaultStats();
    } catch (e) {
      _error = '$e';
    }
    notifyListeners();
  }

  void _replace(SaveView updated) {
    _saves = [
      for (final save in _saves) save.id == updated.id ? updated : save,
    ];
    if (selection.value?.id == updated.id) selection.value = updated;
  }

  @override
  void dispose() {
    criteria.dispose();
    statsView.dispose();
    tagCounts.dispose();
    selection.dispose();
    gridView.dispose();
    super.dispose();
  }

  /// Clear a surfaced error once the UI has shown it, so the same failure is
  /// not reported twice on the next rebuild.
  void clearError() {
    if (_error == null) return;
    _error = null;
    notifyListeners();
  }
}
