// One save, in full.
//
// The same widget serves the desktop detail pane and the pushed mobile
// route, because everything that differs between them is chrome: the pane
// has no app bar and no back button, the route has both. That is handed in
// rather than detected, so the widget itself stays layout-agnostic.

import 'package:flutter/material.dart';

import '../src/rust/api/vault_api.dart';
import '../theme/websave_theme.dart';
import 'save_row.dart' show confirmDelete, openInBrowser;
import 'saves_controller.dart';

class SaveDetail extends StatelessWidget {
  const SaveDetail({
    super.key,
    required this.save,
    required this.controller,
    this.onClose,
  });

  final SaveView save;
  final SavesController controller;

  /// Supplied by the desktop pane (clears the selection). The pushed route
  /// leaves this null and relies on the back button.
  final VoidCallback? onClose;

  @override
  Widget build(BuildContext context) {
    final ws = context.ws;
    final host = Uri.tryParse(save.url)?.host ?? save.url;

    return Container(
      color: ws.background,
      child: ListView(
        padding: EdgeInsets.all(ws.cardPadding * 1.5),
        children: [
          Row(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Expanded(
                child: Text(
                  save.title.isEmpty ? host : save.title,
                  style: TextStyle(
                    color: ws.text,
                    fontSize: 18,
                    fontWeight: FontWeight.w700,
                    height: 1.25,
                  ),
                ),
              ),
              IconButton(
                icon: const Icon(Icons.delete_outline, size: 17),
                color: ws.textDim,
                tooltip: 'Delete',
                onPressed: () => confirmDelete(context, save, controller),
              ),
              if (onClose != null)
                IconButton(
                  icon: const Icon(Icons.close, size: 17),
                  color: ws.textDim,
                  onPressed: onClose,
                  tooltip: 'Close',
                ),
            ],
          ),
          const SizedBox(height: 6),
          SelectableText(
            save.url,
            style: TextStyle(color: ws.textDim, fontSize: 12.5),
          ),
          const SizedBox(height: 14),
          _StatusLine(save: save),
          if (save.description.isNotEmpty) ...[
            const SizedBox(height: 16),
            Text(
              save.description,
              style: TextStyle(color: ws.text, fontSize: 13.5, height: 1.5),
            ),
          ],
          if (save.tags.isNotEmpty) ...[
            const SizedBox(height: 16),
            Wrap(
              spacing: 6,
              runSpacing: 6,
              children: [for (final tag in save.tags) _TagChip(tag: tag)],
            ),
          ],
          const SizedBox(height: 20),
          _Actions(save: save, controller: controller),
          const SizedBox(height: 20),
          Divider(color: ws.border),
          const SizedBox(height: 12),
          _MetaGrid(save: save),
        ],
      ),
    );
  }
}

class _StatusLine extends StatelessWidget {
  const _StatusLine({required this.save});
  final SaveView save;

  @override
  Widget build(BuildContext context) {
    final ws = context.ws;
    final color = ws.health.of(save.status);
    return Row(
      children: [
        Container(
          padding: const EdgeInsets.symmetric(horizontal: 9, vertical: 4),
          decoration: BoxDecoration(
            // A tint of the health colour rather than the colour itself, so
            // a "dead" badge reads as information and not as an alarm.
            color: color.withValues(alpha: 0.14),
            borderRadius: BorderRadius.circular(99),
          ),
          child: Row(
            mainAxisSize: MainAxisSize.min,
            children: [
              Container(
                width: 7,
                height: 7,
                decoration: BoxDecoration(color: color, shape: BoxShape.circle),
              ),
              const SizedBox(width: 6),
              Text(
                save.status,
                style: TextStyle(
                  color: color,
                  fontSize: 11.5,
                  fontWeight: FontWeight.w600,
                ),
              ),
            ],
          ),
        ),
        if (save.httpStatus != null) ...[
          const SizedBox(width: 8),
          Text(
            'HTTP ${save.httpStatus}',
            style: TextStyle(color: ws.textDim, fontSize: 11.5),
          ),
        ],
      ],
    );
  }
}

class _TagChip extends StatelessWidget {
  const _TagChip({required this.tag});
  final String tag;

  @override
  Widget build(BuildContext context) {
    final ws = context.ws;
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 3),
      decoration: BoxDecoration(
        color: ws.panel,
        border: Border.all(color: ws.border),
        borderRadius: BorderRadius.circular(99),
      ),
      child: Text(tag, style: TextStyle(color: ws.textDim, fontSize: 11.5)),
    );
  }
}

class _Actions extends StatelessWidget {
  const _Actions({required this.save, required this.controller});
  final SaveView save;
  final SavesController controller;

  @override
  Widget build(BuildContext context) {
    final ws = context.ws;
    final checking = controller.isChecking(save.id);
    return Wrap(
      spacing: 8,
      runSpacing: 8,
      children: [
        FilledButton.icon(
          onPressed: () => openInBrowser(save.url),
          icon: const Icon(Icons.open_in_new, size: 15),
          label: const Text('Open'),
          style: FilledButton.styleFrom(
            backgroundColor: ws.accent,
            foregroundColor: ws.accentText,
            minimumSize: Size(0, ws.hitTarget + 6),
            shape: RoundedRectangleBorder(
              borderRadius: BorderRadius.circular(ws.controlRadius),
            ),
          ),
        ),
        OutlinedButton.icon(
          onPressed: () => controller.toggleRead(save),
          icon: Icon(
            save.isRead ? Icons.mark_email_unread_outlined : Icons.done,
            size: 15,
          ),
          label: Text(save.isRead ? 'Mark unread' : 'Mark read'),
          style: OutlinedButton.styleFrom(
            foregroundColor: ws.text,
            side: BorderSide(color: ws.border),
            minimumSize: Size(0, ws.hitTarget + 6),
            shape: RoundedRectangleBorder(
              borderRadius: BorderRadius.circular(ws.controlRadius),
            ),
          ),
        ),
        OutlinedButton.icon(
          onPressed: () => controller.toggleFavorite(save),
          icon: Icon(save.favorite ? Icons.star : Icons.star_outline, size: 15),
          label: Text(save.favorite ? 'Starred' : 'Star'),
          style: OutlinedButton.styleFrom(
            foregroundColor: ws.text,
            side: BorderSide(color: ws.border),
            minimumSize: Size(0, ws.hitTarget + 6),
            shape: RoundedRectangleBorder(
              borderRadius: BorderRadius.circular(ws.controlRadius),
            ),
          ),
        ),
        OutlinedButton.icon(
          // Disabled while in flight rather than hidden, so the button does
          // not move under the cursor mid-check.
          onPressed: checking ? null : () => controller.checkNow(save),
          icon: checking
              ? SizedBox(
                  width: 13,
                  height: 13,
                  child: CircularProgressIndicator(
                    strokeWidth: 1.6,
                    color: ws.textDim,
                  ),
                )
              : const Icon(Icons.refresh, size: 15),
          label: Text(checking ? 'Checking…' : 'Check now'),
          style: OutlinedButton.styleFrom(
            foregroundColor: ws.text,
            side: BorderSide(color: ws.border),
            minimumSize: Size(0, ws.hitTarget + 6),
            shape: RoundedRectangleBorder(
              borderRadius: BorderRadius.circular(ws.controlRadius),
            ),
          ),
        ),
      ],
    );
  }
}

class _MetaGrid extends StatelessWidget {
  const _MetaGrid({required this.save});
  final SaveView save;

  @override
  Widget build(BuildContext context) {
    final rows = <(String, String)>[
      ('Saved', _formatDate(save.createdAt.toInt())),
      ('Last checked', _formatDate(save.lastCheckedAt?.toInt())),
      ('Read', save.isRead ? 'Yes' : 'No'),
    ];
    return Column(
      children: [
        for (final (label, value) in rows) _MetaRow(label: label, value: value),
      ],
    );
  }

  static String _formatDate(int? epochSeconds) {
    if (epochSeconds == null || epochSeconds == 0) return 'Never';
    final at = DateTime.fromMillisecondsSinceEpoch(epochSeconds * 1000);
    final now = DateTime.now();
    final days = now.difference(at).inDays;
    if (days == 0) return 'Today';
    if (days == 1) return 'Yesterday';
    if (days < 30) return '$days days ago';
    return '${at.year}-${at.month.toString().padLeft(2, '0')}-'
        '${at.day.toString().padLeft(2, '0')}';
  }
}

class _MetaRow extends StatelessWidget {
  const _MetaRow({required this.label, required this.value});
  final String label;
  final String value;

  @override
  Widget build(BuildContext context) {
    final ws = context.ws;
    return Padding(
      padding: const EdgeInsets.only(bottom: 7),
      child: Row(
        children: [
          SizedBox(
            width: 110,
            child: Text(
              label,
              style: TextStyle(color: ws.textDim, fontSize: 12.5),
            ),
          ),
          Text(value, style: TextStyle(color: ws.text, fontSize: 12.5)),
        ],
      ),
    );
  }
}

/// The pushed variant, for layouts with no room for a pane.
class SaveDetailPage extends StatelessWidget {
  const SaveDetailPage({
    super.key,
    required this.save,
    required this.controller,
  });

  final SaveView save;
  final SavesController controller;

  @override
  Widget build(BuildContext context) {
    // Rebuilds with the controller so a write made from this route — or a
    // check finishing — is reflected without popping back to the list.
    return ListenableBuilder(
      listenable: controller,
      builder: (context, _) {
        final current = controller.saves.firstWhere(
          (s) => s.id == save.id,
          orElse: () => save,
        );
        return Scaffold(
          backgroundColor: context.ws.background,
          appBar: AppBar(title: const Text('Save')),
          body: SaveDetail(save: current, controller: controller),
        );
      },
    );
  }
}
