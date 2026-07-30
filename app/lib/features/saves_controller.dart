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

class SavesController extends ChangeNotifier {
  List<SaveView> _saves = const [];
  StatsView? _stats;
  SaveView? _selected;
  String _query = '';
  bool _loading = true;
  String? _error;

  /// Ids with a check in flight, so a row can show a spinner without the
  /// whole list going into a loading state.
  final Set<int> _checking = <int>{};

  List<SaveView> get saves => _saves;
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
        listSaves(query: _query.isEmpty ? null : _query, limit: 500),
        vaultStats(),
      ]);
      _saves = results[0] as List<SaveView>;
      _stats = results[1] as StatsView;
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

  void select(SaveView? save) {
    _selected = save;
    notifyListeners();
  }

  // ---- writes ----

  Future<void> toggleFavorite(SaveView save) =>
      _apply(() => setFavorite(id: save.id, favorite: !save.favorite));

  Future<void> toggleRead(SaveView save) =>
      _apply(() => setRead(id: save.id, isRead: !save.isRead));

  Future<void> rename(SaveView save, String title) =>
      _apply(() => updateSave(id: save.id, title: title));

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
