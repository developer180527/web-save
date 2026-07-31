// Shared chrome for the settings panel.
//
// Settings grew from a flat column of two headings into five groups, which is
// past the point where a screen can be read by scanning bold text. These give
// every group the same shape — title, one line of explanation, a bordered body
// — so the eye can skip a whole section rather than reading down it.

import 'package:flutter/material.dart';

import '../theme/websave_theme.dart';

class SettingsSection extends StatelessWidget {
  const SettingsSection({
    super.key,
    required this.title,
    required this.children,
    this.description,
  });

  final String title;

  /// One line on why the section exists, not what its controls do. Skipped
  /// where the title already says it.
  final String? description;

  final List<Widget> children;

  @override
  Widget build(BuildContext context) {
    final ws = context.ws;
    return Padding(
      padding: const EdgeInsets.only(bottom: 28),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Text(
            title,
            style: TextStyle(
              color: ws.text,
              fontSize: 15,
              fontWeight: FontWeight.w600,
            ),
          ),
          if (description case final text?) ...[
            const SizedBox(height: 4),
            Text(
              text,
              style: TextStyle(color: ws.textDim, fontSize: 12.5, height: 1.4),
            ),
          ],
          const SizedBox(height: 12),
          ...children,
        ],
      ),
    );
  }
}

/// A labelled control on its own row: explanation on the left, control right.
///
/// Kept to one line of label and one of detail. A setting needing more than
/// that is usually two settings, or belongs behind its own screen.
class SettingsRow extends StatelessWidget {
  const SettingsRow({
    super.key,
    required this.label,
    required this.trailing,
    this.detail,
    this.enabled = true,
  });

  final String label;
  final String? detail;
  final Widget trailing;

  /// Dims the text when the control cannot be used, so a disabled switch does
  /// not read as merely switched off.
  final bool enabled;

  @override
  Widget build(BuildContext context) {
    final ws = context.ws;
    return Padding(
      padding: const EdgeInsets.only(bottom: 10),
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text(
                  label,
                  style: TextStyle(
                    color: enabled ? ws.text : ws.textDim,
                    fontSize: 13.5,
                  ),
                ),
                if (detail case final text?)
                  Padding(
                    padding: const EdgeInsets.only(top: 2),
                    child: Text(
                      text,
                      style: TextStyle(
                        color: ws.textDim,
                        fontSize: 12,
                        height: 1.35,
                      ),
                    ),
                  ),
              ],
            ),
          ),
          const SizedBox(width: 16),
          trailing,
        ],
      ),
    );
  }
}
