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

class SavesController extends ChangeNotifier {
  List<SaveView> _saves = const [];
  List<TagCountView> _tags = const [];
  StatsView? _stats;
  SaveView? _selected;
  String _query = '';
  SaveFilter _filter = SaveFilter.all;
  String? _tag;
  bool _gridView = true;
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
  List<TagCountView> get tags => _tags;
  SaveFilter get filter => _filter;
  String? get activeTag => _tag;
  bool get isGridView => _gridView;
  StatsView? get stats => _stats;
  SaveView? get selected => _selected;
  String get query => _query;
  bool get isLoading => _loading;
  String? get error => _error;
  bool isChecking(int id) => _checking.contains(id);

  Future<void> load() async {
    _loading = true;
    _error = null;
    notifyListeners();
    try {
      final results = await Future.wait([
        listSaves(
          query: _query.isEmpty ? null : _query,
          tag: _tag,
          status: _filter.status,
          favoritesOnly: _filter.favoritesOnly,
          unreadOnly: _filter.unreadOnly,
          limit: 2000,
        ),
        vaultStats(),
        listTags(),
      ]);
      _saves = results[0] as List<SaveView>;
      _stats = results[1] as StatsView;
      _tags = results[2] as List<TagCountView>;
      _thumbnailRoot ??= await assetsDir();
      // A save can fall out of the result set when the query changes; a
      // selection pointing at a row that is no longer listed would leave the
      // detail pane showing something the list cannot highlight.
      if (_selected != null && !_saves.any((s) => s.id == _selected!.id)) {
        _selected = null;
      }
    } catch (e) {
      _error = '$e';
    } finally {
      _loading = false;
      notifyListeners();
    }
  }

  Future<void> search(String query) {
    _query = query;
    return load();
  }

  /// Picking a section clears any tag narrowing, and vice versa — two
  /// simultaneous narrowings with only one of them visible in the sidebar is
  /// how people end up staring at an empty list.
  Future<void> setFilter(SaveFilter filter) {
    _filter = filter;
    _tag = null;
    return load();
  }

  Future<void> setTag(String? tag) {
    _tag = tag;
    _filter = SaveFilter.all;
    return load();
  }

  void toggleView() {
    _gridView = !_gridView;
    notifyListeners();
  }

  void select(SaveView? save) {
    _selected = save;
    notifyListeners();
  }

  // ---- writes ----

  Future<void> toggleFavorite(SaveView save) =>
      _apply(() => setFavorite(id: save.id, favorite: !save.favorite));

  Future<void> toggleRead(SaveView save) =>
      _apply(() => setRead(id: save.id, isRead: !save.isRead));

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
      _stats = await vaultStats();
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
      if (_selected?.id == save.id) _selected = null;
      _stats = await vaultStats();
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
      _stats = await vaultStats();
    } catch (e) {
      _error = '$e';
    }
    notifyListeners();
  }

  void _replace(SaveView updated) {
    _saves = [
      for (final save in _saves) save.id == updated.id ? updated : save,
    ];
    if (_selected?.id == updated.id) _selected = updated;
  }

  /// Clear a surfaced error once the UI has shown it, so the same failure is
  /// not reported twice on the next rebuild.
  void clearError() {
    if (_error == null) return;
    _error = null;
    notifyListeners();
  }
}
