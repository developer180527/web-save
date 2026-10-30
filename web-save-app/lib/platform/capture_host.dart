// The browser extension's way in: the capture server on 127.0.0.1:38917.
//
// The server is core's; the bridge starts it against this app's vault. This
// file decides when (desktop only, once the vault is open), keeps the result
// where Settings can show it, and turns the server's events into a refresh
// or a raised window.

import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:package_info_plus/package_info_plus.dart';
import 'package:window_manager/window_manager.dart';

import '../src/rust/api/capture_api.dart';
import 'desktop_window.dart';

/// How starting the capture server went, or null before it has been tried
/// (and always on mobile, where there is no browser extension to serve).
final captureServer = ValueNotifier<CaptureServerStart?>(null);

/// Start serving the extension and react to what it delivers.
///
/// [onVaultChanged] runs when a capture or a companion app wrote to the
/// vault, so the visible lists can reload. Returns the event subscription,
/// for the caller to cancel.
Future<StreamSubscription<CaptureNotice>?> startCaptureHost({
  required VoidCallback onVaultChanged,
}) async {
  if (!isDesktop) return null;

  final subscription = captureEvents().listen((notice) async {
    switch (notice.kind) {
      case CaptureNoticeKind.saved:
      case CaptureNoticeKind.reloadRequested:
        onVaultChanged();
      case CaptureNoticeKind.showRequested:
        // The macOS menubar's "Open WebSave".
        await windowManager.show();
        await windowManager.focus();
    }
  });

  try {
    final version = (await PackageInfo.fromPlatform()).version;
    captureServer.value = await startCaptureServer(appVersion: version);
  } catch (e) {
    captureServer.value = CaptureServerStart(
      state: CaptureServerState.unavailable,
      reason: '$e',
    );
  }
  return subscription;
}
