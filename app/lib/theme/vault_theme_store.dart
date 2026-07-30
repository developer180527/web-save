// Persist the theme choice in the vault, not in platform preferences.
//
// The vault is a plain directory the user owns and can move, back up or sync.
// Putting the selection in its `meta` table means the choice travels with the
// data: open the same vault on a Mac and an iPad and both land on the same
// theme, with no account and no sync service. SharedPreferences would strand
// it per-device and per-install.
//
// The meta table already exists for exactly this kind of host-defined key --
// core attaches no meaning to any of them.

import 'package:flutter_theme_engine/flutter_theme_engine.dart';

import '../src/rust/api/vault_api.dart';

class VaultThemeStore implements ThemeStore {
  const VaultThemeStore();

  /// Namespaced so it can never collide with the extension's keys.
  static const _key = 'theme.selected';

  @override
  Future<String?> read() async {
    try {
      final value = await getPref(key: _key);
      return (value == null || value.isEmpty) ? null : value;
    } catch (_) {
      // A vault that isn't open yet, or a read failure, must not stop the app
      // from starting — it just falls back to the default pack.
      return null;
    }
  }

  @override
  Future<void> write(String id) async {
    try {
      await setPref(key: _key, value: id);
    } catch (_) {
      // Losing a theme preference is not worth surfacing an error for.
    }
  }

  @override
  Future<void> clear() async {
    try {
      await setPref(key: _key, value: '');
    } catch (_) {}
  }
}
