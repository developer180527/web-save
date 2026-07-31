// Getting data in and out of the vault.
//
// On iOS this is the only way in: nothing on a phone can reach the capture
// server on 127.0.0.1, and there is no Share Extension yet. The intended
// round trip is export on the Mac, AirDrop the file, import here.
//
// Import is two steps on purpose. preview_import parses the file and asks the
// vault what *would* happen without writing anything, so the count of new
// versus already-here versus unusable is shown before anyone commits. That
// matters because importing the wrong file into a bookmark collection is
// tedious to undo by hand.

import 'dart:io';

import 'package:file_selector/file_selector.dart';
import 'package:flutter/material.dart';
import 'package:path_provider/path_provider.dart';
import 'package:share_plus/share_plus.dart';

import '../src/rust/api/vault_api.dart';
import '../theme/websave_theme.dart';
import 'saves_controller.dart';

/// Import and export as a standalone screen, for shells that route to it as
/// a destination.
class TransferScreen extends StatelessWidget {
  const TransferScreen({super.key, required this.controller});

  final SavesController controller;

  @override
  Widget build(BuildContext context) => ListView(
    padding: EdgeInsets.all(context.ws.cardPadding * 1.5),
    children: [TransferPanel(controller: controller)],
  );
}

/// The same controls with no scroll view or padding of its own, so Settings
/// can place it inside a section without nesting one list inside another.
class TransferPanel extends StatefulWidget {
  const TransferPanel({super.key, required this.controller});

  final SavesController controller;

  @override
  State<TransferPanel> createState() => _TransferPanelState();
}

class _TransferPanelState extends State<TransferPanel> {
  String? _pickedName;
  String? _pickedContent;
  ImportPreview? _preview;
  ImportPreview? _result;
  String? _error;
  bool _busy = false;

  Future<void> _pick() async {
    setState(() {
      _error = null;
      _result = null;
    });
    try {
      // Deliberately unfiltered: AirDrop and Files hand over inconsistent
      // types, and core's parser sniffs the content rather than trusting the
      // name, so filtering by extension would mostly hide valid files.
      final file = await openFile();
      if (file == null) return;
      setState(() => _busy = true);
      final content = await file.readAsString();
      final preview = await previewImport(content: content);
      setState(() {
        _pickedName = file.name;
        _pickedContent = content;
        _preview = preview;
      });
    } catch (e) {
      setState(() => _error = '$e');
    } finally {
      if (mounted) setState(() => _busy = false);
    }
  }

  Future<void> _import() async {
    final content = _pickedContent;
    if (content == null) return;
    setState(() => _busy = true);
    try {
      final result = await runImport(content: content);
      // The list and every sidebar count just moved.
      await widget.controller.load();
      setState(() {
        _result = result;
        _preview = null;
        _pickedContent = null;
      });
    } catch (e) {
      setState(() => _error = '$e');
    } finally {
      if (mounted) setState(() => _busy = false);
    }
  }

  Future<void> _export(String format) async {
    setState(() {
      _busy = true;
      _error = null;
    });
    try {
      final content = await exportVault(format: format);
      final stamp = DateTime.now().toIso8601String().split('T').first;
      final extension = format == 'json' ? 'json' : 'html';
      final dir = await getTemporaryDirectory();
      final file = File('${dir.path}/websave-$stamp.$extension');
      await file.writeAsString(content);
      // The system share sheet, so the file can go to Files, AirDrop, mail —
      // wherever. On desktop this saves through the same sheet.
      await SharePlus.instance.share(
        ShareParams(files: [XFile(file.path)], text: 'WebSave export'),
      );
    } catch (e) {
      setState(() => _error = '$e');
    } finally {
      if (mounted) setState(() => _busy = false);
    }
  }

  @override
  Widget build(BuildContext context) {
    final ws = context.ws;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        _Heading('Import'),
        Text(
          'A file exported from WebSave, or a bookmarks file from any '
          'browser.',
          style: TextStyle(color: ws.textDim, fontSize: 12.5),
        ),
        const SizedBox(height: 12),
        _Button(
          icon: Icons.folder_open,
          label: _pickedName == null
              ? 'Choose a file…'
              : 'Choose another file…',
          onTap: _busy ? null : _pick,
        ),
        if (_pickedName != null) ...[
          const SizedBox(height: 8),
          Text(_pickedName!, style: TextStyle(color: ws.textDim, fontSize: 12)),
        ],
        if (_preview case final preview?) ...[
          const SizedBox(height: 12),
          _PreviewCard(preview: preview),
          const SizedBox(height: 10),
          _Button(
            icon: Icons.download_done,
            label: preview.newItems == 0
                ? 'Nothing new to import'
                : 'Import ${preview.newItems} new save(s)',
            primary: true,
            onTap: (_busy || preview.newItems == 0) ? null : _import,
          ),
        ],
        if (_result case final result?) ...[
          const SizedBox(height: 12),
          _ResultCard(result: result),
        ],
        const SizedBox(height: 24),
        _Heading('Export'),
        Text(
          'HTML is the round trip: browsers read it and so does this app\'s '
          'own importer. JSON carries more detail — link health, timestamps — '
          'but is for backup rather than for moving a vault.',
          style: TextStyle(color: ws.textDim, fontSize: 12.5),
        ),
        const SizedBox(height: 12),
        Row(
          children: [
            _Button(
              icon: Icons.ios_share,
              label: 'Export HTML',
              onTap: _busy ? null : () => _export('html'),
            ),
            const SizedBox(width: 8),
            _Button(
              icon: Icons.data_object,
              label: 'Export JSON',
              onTap: _busy ? null : () => _export('json'),
            ),
          ],
        ),
        if (_error case final error?) ...[
          const SizedBox(height: 16),
          Text(error, style: TextStyle(color: ws.danger, fontSize: 12.5)),
        ],
        if (_busy) ...[
          const SizedBox(height: 16),
          const Center(child: CircularProgressIndicator()),
        ],
      ],
    );
  }
}

class _PreviewCard extends StatelessWidget {
  const _PreviewCard({required this.preview});
  final ImportPreview preview;

  @override
  Widget build(BuildContext context) {
    final ws = context.ws;
    return Container(
      padding: EdgeInsets.all(ws.cardPadding),
      decoration: BoxDecoration(
        color: ws.panel,
        border: Border.all(color: ws.border),
        borderRadius: BorderRadius.circular(ws.cardRadius),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Text(
            preview.format,
            style: TextStyle(
              color: ws.text,
              fontSize: 13,
              fontWeight: FontWeight.w600,
            ),
          ),
          const SizedBox(height: 8),
          _Stat(label: 'New', value: preview.newItems, color: ws.health.active),
          _Stat(
            label: 'Already saved',
            value: preview.existing,
            color: ws.textDim,
          ),
          if (preview.invalid > 0)
            _Stat(
              label: 'Unusable',
              value: preview.invalid,
              color: ws.health.dead,
            ),
          const SizedBox(height: 6),
          Text(
            'Existing saves are merged, never overwritten — importing the '
            'same file twice is safe.',
            style: TextStyle(color: ws.textDim, fontSize: 11.5),
          ),
        ],
      ),
    );
  }
}

class _ResultCard extends StatelessWidget {
  const _ResultCard({required this.result});
  final ImportPreview result;

  @override
  Widget build(BuildContext context) {
    final ws = context.ws;
    return Container(
      padding: EdgeInsets.all(ws.cardPadding),
      decoration: BoxDecoration(
        color: ws.health.active.withValues(alpha: 0.12),
        borderRadius: BorderRadius.circular(ws.cardRadius),
      ),
      child: Text(
        'Imported ${result.newItems} new save(s). '
        '${result.existing} were already here.',
        style: TextStyle(color: ws.text, fontSize: 13),
      ),
    );
  }
}

class _Stat extends StatelessWidget {
  const _Stat({required this.label, required this.value, required this.color});
  final String label;
  final int value;
  final Color color;

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.only(bottom: 3),
      child: Row(
        children: [
          Container(
            width: 7,
            height: 7,
            decoration: BoxDecoration(color: color, shape: BoxShape.circle),
          ),
          const SizedBox(width: 8),
          Text(
            '$value  $label',
            style: TextStyle(color: context.ws.text, fontSize: 12.5),
          ),
        ],
      ),
    );
  }
}

class _Heading extends StatelessWidget {
  const _Heading(this.text);
  final String text;

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.only(bottom: 4),
      child: Text(
        text,
        style: TextStyle(
          color: context.ws.text,
          fontSize: 15,
          fontWeight: FontWeight.w600,
        ),
      ),
    );
  }
}

class _Button extends StatelessWidget {
  const _Button({
    required this.icon,
    required this.label,
    required this.onTap,
    this.primary = false,
  });

  final IconData icon;
  final String label;
  final VoidCallback? onTap;
  final bool primary;

  @override
  Widget build(BuildContext context) {
    final ws = context.ws;
    final style = ButtonStyle(
      minimumSize: WidgetStatePropertyAll(Size(0, ws.hitTarget + 8)),
      shape: WidgetStatePropertyAll(
        RoundedRectangleBorder(
          borderRadius: BorderRadius.circular(ws.controlRadius),
        ),
      ),
    );
    return primary
        ? FilledButton.icon(
            onPressed: onTap,
            icon: Icon(icon, size: 15),
            label: Text(label),
            style: style.copyWith(
              backgroundColor: WidgetStatePropertyAll(ws.accent),
              foregroundColor: WidgetStatePropertyAll(ws.accentText),
            ),
          )
        : OutlinedButton.icon(
            onPressed: onTap,
            icon: Icon(icon, size: 15),
            label: Text(label),
            style: style.copyWith(
              foregroundColor: WidgetStatePropertyAll(ws.text),
              side: WidgetStatePropertyAll(BorderSide(color: ws.border)),
            ),
          );
  }
}
