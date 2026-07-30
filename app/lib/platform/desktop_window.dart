// Window chrome setup, desktop only.
//
// Mirrors what the Tauri app does with tauri-plugin-decorum: on macOS the
// native traffic lights stay, overlaid on our own bar and nudged to sit
// centred in it; on Windows and Linux the frame is removed entirely and the
// title bar draws its own controls.
//
// Guarded so the whole file is inert on iOS and Android, where there is no
// window to configure -- `window_manager` would throw if called there.

import 'dart:io' show Platform;

import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart' show Color, Size;
import 'package:window_manager/window_manager.dart';

/// True on the three desktop platforms. Checked against `Platform` rather
/// than `Theme.of(context).platform` because this runs before any widget.
bool get isDesktop =>
    !kIsWeb && (Platform.isMacOS || Platform.isWindows || Platform.isLinux);

/// Height of the custom title bar. Matches the React app's 38px.
const double kTitleBarHeight = 38;

/// Configure the window. Safe to call on any platform; does nothing off
/// desktop.
Future<void> setUpDesktopWindow() async {
  if (!isDesktop) return;
  await windowManager.ensureInitialized();

  await windowManager.waitUntilReadyToShow(
    const WindowOptions(
      size: Size(1180, 760),
      // Below this the rail plus a usable list stops fitting. The shell would
      // still cope, but nothing good happens down there.
      minimumSize: Size(560, 420),
      center: true,
      backgroundColor: Color(0x00000000),
      skipTaskbar: false,
      // macOS keeps its traffic lights and hides only the bar; the others go
      // fully frameless and the Flutter title bar draws its own buttons.
      titleBarStyle: TitleBarStyle.hidden,
      windowButtonVisibility: true,
    ),
    () async {
      await windowManager.show();
      await windowManager.focus();
    },
  );
}
