// Settings, in five groups: Appearance, General, Library, Vault, and the
// browser extension.
//
// Library is also the only way to reach import and export on desktop. The
// desktop sidebar is the vault's filters, not the app's destinations, so a
// top-level Import screen exists in the shell but has nothing to open it —
// putting it here gives it a home on every platform rather than a second
// navigation surface competing with the filters.

import '../theme/app_icons.dart';
import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter_theme_engine/flutter_theme_engine.dart';

import 'package:package_info_plus/package_info_plus.dart';

import '../platform/capture_host.dart';
import '../src/rust/api/capture_api.dart';
import '../src/rust/api/vault_api.dart';
import '../theme/websave_theme.dart';
import 'activity_log_panel.dart';
import 'saves_controller.dart';
import 'settings_section.dart';
import 'transfer_screen.dart';

class SettingsScreen extends StatelessWidget {
  const SettingsScreen({
    super.key,
    required this.vaultPath,
    required this.controller,
  });

  final String vaultPath;
  final SavesController controller;

  /// Login items and a menubar companion only mean something on a desktop.
  static bool get _isDesktop =>
      Platform.isMacOS || Platform.isWindows || Platform.isLinux;

  @override
  Widget build(BuildContext context) {
    final ws = context.ws;
    final engine = ThemeEngineScope.of<WebSaveTheme>(context);
    final packs = engine.packs.where((p) => !p.id.endsWith('_touch')).toList();

    return ListView(
      padding: EdgeInsets.all(ws.cardPadding * 1.5),
      children: [
        SettingsSection(
          title: 'Appearance',
          description:
              'Themes are stored in the vault, so the choice travels with '
              'your data rather than living on this device.',
          children: [
            for (final pack in packs)
              _ThemeRow(
                pack: pack,
                selected:
                    engine.currentId == pack.id ||
                    engine.currentId == '${pack.id}_touch',
                onTap: () => engine.apply(pack.id),
              ),
            const SizedBox(height: 2),
            TextButton(
              onPressed: engine.reset,
              child: const Text('Follow the system appearance'),
            ),
          ],
        ),
        if (_isDesktop)
          const SettingsSection(
            title: 'General',
            description:
                'How WebSave behaves on this machine. These stay on the '
                'device rather than in the vault, since they describe the '
                'computer and not the collection.',
            children: [_LaunchAtLogin(), _MenubarPlaceholder()],
          ),
        SettingsSection(
          title: 'Library',
          description:
              'Move saves in and out. Nothing is written until a preview has '
              'shown what an import would change.',
          children: [TransferPanel(controller: controller)],
        ),
        SettingsSection(
          title: 'Vault',
          description: 'Where this app keeps its data.',
          children: [
            SelectableText(
              vaultPath,
              style: TextStyle(color: ws.textDim, fontSize: 12, height: 1.4),
            ),
          ],
        ),
        SettingsSection(
          title: 'Browser extension',
          description:
              'Saving straight from the browser, without going through the '
              'app.',
          children: [
            if (_isDesktop)
              const _ExtensionStatusPanel()
            else
              const _NotYet(
                'The extension talks to the capture server on a desktop, so '
                'there is nothing to report here.',
              ),
          ],
        ),
        const SettingsSection(
          title: 'Activity',
          description:
              'What the app has been doing, including work done in the '
              'background where there is no window to report it.',
          children: [ActivityLogPanel()],
        ),
        const SettingsSection(title: 'About', children: [_AboutPanel()]),
      ],
    );
  }
}

/// The login-item switch.
///
/// Reads its state from disk on every build rather than trusting a stored
/// preference, so an entry removed in System Settings shows here as off. The
/// write returns the state read back afterwards for the same reason:
/// registration can fail quietly under a managed profile, and a switch that
/// moves anyway would be lying.
class _LaunchAtLogin extends StatefulWidget {
  const _LaunchAtLogin();

  @override
  State<_LaunchAtLogin> createState() => _LaunchAtLoginState();
}

class _LaunchAtLoginState extends State<_LaunchAtLogin> {
  bool? _enabled;
  bool _busy = false;
  String? _error;

  @override
  void initState() {
    super.initState();
    launchAtLogin()
        .then((v) => mounted ? setState(() => _enabled = v) : null)
        .catchError((Object e) {
          if (mounted) setState(() => _error = '$e');
        });
  }

  Future<void> _set(bool value) async {
    setState(() {
      _busy = true;
      _error = null;
    });
    try {
      final actual = await setLaunchAtLogin(enabled: value);
      if (mounted) setState(() => _enabled = actual);
    } catch (e) {
      if (mounted) setState(() => _error = '$e');
    } finally {
      if (mounted) setState(() => _busy = false);
    }
  }

  @override
  Widget build(BuildContext context) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        SettingsRow(
          label: 'Open WebSave at login',
          detail:
              _error ??
              'Registers a login item for this copy of the app. The Tauri '
                  'build registers its own, so leave that one off to avoid '
                  'both starting.',
          enabled: !_busy && _enabled != null,
          trailing: Switch(
            value: _enabled ?? false,
            onChanged: (_busy || _enabled == null) ? null : _set,
          ),
        ),
      ],
    );
  }
}

class _MenubarPlaceholder extends StatelessWidget {
  const _MenubarPlaceholder();

  @override
  Widget build(BuildContext context) => const _NotYet(
    'The menubar companion ships inside the Tauri bundle and is launched from '
    'there. Starting it from this app is not wired up yet.',
  );
}

/// What the vault records about the extension.
///
/// Two separate facts. Whether the extension *can* reach a vault right now
/// (this app's capture server, or another WebSave holding the port), and the
/// last contact that actually happened, which the server stamps into the
/// vault on every call. The second says "connected" off a 12-hour window on
/// purpose: the extension only calls in when something is saved, so a
/// shorter window would report a working setup as broken over a quiet
/// afternoon.
class _ExtensionStatusPanel extends StatefulWidget {
  const _ExtensionStatusPanel();

  @override
  State<_ExtensionStatusPanel> createState() => _ExtensionStatusPanelState();
}

class _ExtensionStatusPanelState extends State<_ExtensionStatusPanel> {
  ExtensionStatus? _status;
  String? _error;

  @override
  void initState() {
    super.initState();
    extensionStatus()
        .then((v) => mounted ? setState(() => _status = v) : null)
        .catchError((Object e) {
          if (mounted) setState(() => _error = '$e');
        });
  }

  String _ago(int seconds) {
    final then = DateTime.fromMillisecondsSinceEpoch(seconds * 1000);
    final gap = DateTime.now().difference(then);
    if (gap.inMinutes < 1) return 'just now';
    if (gap.inHours < 1) return '${gap.inMinutes} min ago';
    if (gap.inDays < 1) return '${gap.inHours} h ago';
    if (gap.inDays < 30) return '${gap.inDays} d ago';
    return '${then.year}-${then.month.toString().padLeft(2, '0')}-'
        '${then.day.toString().padLeft(2, '0')}';
  }

  @override
  Widget build(BuildContext context) {
    if (_error case final message?) return _NotYet(message);
    final status = _status;
    if (status == null) {
      return const SizedBox(
        height: 40,
        child: Center(child: CircularProgressIndicator(strokeWidth: 2)),
      );
    }
    final ws = context.ws;
    final never = status.lastSeen == null;
    return Column(
      children: [
        ValueListenableBuilder(
          valueListenable: captureServer,
          builder: (context, server, _) {
            final (text, detail, ok) = switch (server?.state) {
              null => ('Starting…', null, true),
              CaptureServerState.serving => (
                'Listening',
                'The extension saves straight into this vault.',
                true,
              ),
              CaptureServerState.otherInstance => (
                'Another WebSave',
                'The legacy WebSave app is serving the extension. Saves '
                    'still reach this vault; quit it and restart this app to '
                    'take over.',
                true,
              ),
              CaptureServerState.unavailable => (
                'Unavailable',
                'Another program holds port 38917, so the extension cannot '
                    'save. ${server?.reason ?? ''}'.trim(),
                false,
              ),
            };
            return SettingsRow(
              label: 'Capture server',
              detail: detail,
              trailing: Row(
                mainAxisSize: MainAxisSize.min,
                children: [
                  Container(
                    width: 8,
                    height: 8,
                    decoration: BoxDecoration(
                      color: ok ? ws.health.active : ws.health.dead,
                      shape: BoxShape.circle,
                    ),
                  ),
                  const SizedBox(width: 7),
                  Text(text, style: TextStyle(color: ws.text, fontSize: 13)),
                ],
              ),
            );
          },
        ),
        SettingsRow(
          label: 'Status',
          detail: never
              ? 'No capture has reached this vault yet.'
              : status.connected
              ? 'Saving to this vault.'
              : 'No capture recently. The extension may be disabled.',
          trailing: Row(
            mainAxisSize: MainAxisSize.min,
            children: [
              Container(
                width: 8,
                height: 8,
                decoration: BoxDecoration(
                  color: status.connected ? ws.health.active : ws.health.dead,
                  shape: BoxShape.circle,
                ),
              ),
              const SizedBox(width: 7),
              Text(
                status.connected ? 'Connected' : 'Not connected',
                style: TextStyle(color: ws.text, fontSize: 13),
              ),
            ],
          ),
        ),
        SettingsRow(
          label: 'Last capture',
          trailing: Text(
            never ? '—' : _ago(status.lastSeen!),
            style: TextStyle(color: ws.textDim, fontSize: 13),
          ),
        ),
        SettingsRow(
          label: 'Extension version',
          trailing: Text(
            status.version ?? '—',
            style: TextStyle(color: ws.textDim, fontSize: 13),
          ),
        ),
      ],
    );
  }
}

/// App version and build number, read from the bundle rather than hard-coded
/// so it cannot drift from what was actually shipped.
class _AboutPanel extends StatefulWidget {
  const _AboutPanel();

  @override
  State<_AboutPanel> createState() => _AboutPanelState();
}

class _AboutPanelState extends State<_AboutPanel> {
  PackageInfo? _info;

  @override
  void initState() {
    super.initState();
    PackageInfo.fromPlatform().then(
      (v) => mounted ? setState(() => _info = v) : null,
    );
  }

  @override
  Widget build(BuildContext context) {
    final ws = context.ws;
    final info = _info;
    return Column(
      children: [
        SettingsRow(
          label: 'Version',
          trailing: Text(
            info?.version ?? '—',
            style: TextStyle(color: ws.textDim, fontSize: 13),
          ),
        ),
        SettingsRow(
          label: 'Build',
          trailing: Text(
            info?.buildNumber ?? '—',
            style: TextStyle(color: ws.textDim, fontSize: 13),
          ),
        ),
      ],
    );
  }
}

class _NotYet extends StatelessWidget {
  const _NotYet(this.text);
  final String text;

  @override
  Widget build(BuildContext context) {
    final ws = context.ws;
    return Container(
      padding: EdgeInsets.all(ws.cardPadding),
      decoration: BoxDecoration(
        border: Border.all(color: ws.border),
        borderRadius: BorderRadius.circular(ws.cardRadius),
      ),
      child: Text(
        text,
        style: TextStyle(color: ws.textDim, fontSize: 12.5, height: 1.4),
      ),
    );
  }
}

class _ThemeRow extends StatelessWidget {
  const _ThemeRow({
    required this.pack,
    required this.selected,
    required this.onTap,
  });

  final ThemePack<WebSaveTheme> pack;
  final bool selected;
  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) {
    final ws = context.ws;
    return Padding(
      padding: const EdgeInsets.only(bottom: 6),
      child: GestureDetector(
        onTap: onTap,
        child: Container(
          padding: EdgeInsets.all(ws.cardPadding * 0.8),
          decoration: BoxDecoration(
            color: selected ? ws.states.selected : ws.panel,
            border: Border.all(color: selected ? ws.accent : ws.border),
            borderRadius: BorderRadius.circular(ws.cardRadius),
          ),
          child: Row(
            children: [
              // The stored swatch, never pack.themeData -- see the note on
              // WebSaveTheme.swatch.
              Container(
                width: 42,
                height: 28,
                decoration: BoxDecoration(
                  borderRadius: BorderRadius.circular(ws.iconTileRadius),
                  border: Border.all(color: ws.border),
                ),
                child: ClipRRect(
                  borderRadius: BorderRadius.circular(ws.iconTileRadius - 1),
                  child: Row(
                    children: [
                      for (final color in pack.model.swatch)
                        Expanded(child: Container(color: color)),
                    ],
                  ),
                ),
              ),
              const SizedBox(width: 12),
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text(
                      pack.meta.name,
                      style: TextStyle(
                        color: ws.text,
                        fontSize: 13.5,
                        fontWeight: FontWeight.w600,
                      ),
                    ),
                    if (pack.meta.description case final description?)
                      Text(
                        description,
                        style: TextStyle(color: ws.textDim, fontSize: 12),
                      ),
                  ],
                ),
              ),
              if (selected) Icon(AppIcons.check, size: 17, color: ws.accent),
            ],
          ),
        ),
      ),
    );
  }
}
