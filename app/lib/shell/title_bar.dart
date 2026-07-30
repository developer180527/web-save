// The custom title bar, drawn only where there is a window to title.
//
// On macOS the native traffic lights are still there, overlaid at the left,
// so the bar leaves room for them and draws nothing of its own. On Windows
// and Linux the frame is gone entirely, so it draws minimise/maximise/close.
// On mobile and iPad it renders nothing at all — there is no window chrome to
// replace, and a fake bar would only steal a row of screen.
//
// The whole bar is a drag handle, which is what makes a frameless window feel
// like a window rather than a kiosk.

import 'dart:io' show Platform;

import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';
import 'package:window_manager/window_manager.dart';

import '../platform/desktop_window.dart';
import '../theme/websave_theme.dart';
import 'layout_scope.dart';

class AppTitleBar extends StatelessWidget {
  const AppTitleBar({super.key, this.title = 'WebSave'});

  final String title;

  /// Space the macOS traffic lights occupy, so the title clears them.
  /// The Tauri app insets them to (16, 13) in a 38px bar; this reserves the
  /// same footprint on the Flutter side.
  static const double _macButtonInset = 78;

  @override
  Widget build(BuildContext context) {
    if (!context.layout.showsCustomTitleBar) return const SizedBox.shrink();

    final ws = context.ws;
    final isMac = !kIsWeb && Platform.isMacOS;

    // The bar sits *above* the shell's Scaffold in the tree, so there is no
    // Material ancestor to inherit text style from — without this, Flutter
    // paints its yellow "unstyled text" underline through the title.
    return Material(
      color: ws.panel,
      child: DragToMoveArea(
        child: Container(
          height: kTitleBarHeight,
          decoration: BoxDecoration(
            color: ws.panel,
            border: Border(bottom: BorderSide(color: ws.border)),
          ),
          child: Row(
            children: [
              SizedBox(width: isMac ? _macButtonInset : 12),
              Text(
                title,
                style: TextStyle(
                  color: ws.textDim,
                  fontSize: 12.5,
                  fontWeight: FontWeight.w600,
                  letterSpacing: 0.2,
                ),
              ),
              const Spacer(),
              // macOS already has its buttons; only the frameless platforms
              // need ours.
              if (!isMac) const _WindowButtons(),
            ],
          ),
        ),
      ),
    );
  }
}

class _WindowButtons extends StatelessWidget {
  const _WindowButtons();

  @override
  Widget build(BuildContext context) {
    return Row(
      children: [
        _WindowButton(icon: Icons.remove, onTap: windowManager.minimize),
        _WindowButton(
          icon: Icons.crop_square,
          onTap: () async {
            if (await windowManager.isMaximized()) {
              await windowManager.unmaximize();
            } else {
              await windowManager.maximize();
            }
          },
        ),
        _WindowButton(
          icon: Icons.close,
          danger: true,
          // Closing hides rather than quits, matching the Tauri app: capture
          // and link monitoring keep running, and quit lives in the tray.
          onTap: windowManager.hide,
        ),
      ],
    );
  }
}

class _WindowButton extends StatefulWidget {
  const _WindowButton({
    required this.icon,
    required this.onTap,
    this.danger = false,
  });

  final IconData icon;
  final VoidCallback onTap;
  final bool danger;

  @override
  State<_WindowButton> createState() => _WindowButtonState();
}

class _WindowButtonState extends State<_WindowButton> {
  bool _hovered = false;

  @override
  Widget build(BuildContext context) {
    final ws = context.ws;
    return MouseRegion(
      onEnter: (_) => setState(() => _hovered = true),
      onExit: (_) => setState(() => _hovered = false),
      child: GestureDetector(
        onTap: widget.onTap,
        child: Container(
          width: 44,
          height: kTitleBarHeight,
          color: _hovered
              ? (widget.danger ? ws.danger : ws.states.hover)
              : Colors.transparent,
          child: Icon(
            widget.icon,
            size: 15,
            color: _hovered && widget.danger ? ws.accentText : ws.textDim,
          ),
        ),
      ),
    );
  }
}
