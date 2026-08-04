// Adding a link by hand.
//
// Every save used to arrive through the browser extension, which left iOS —
// no extension, no capture server — able only to look at a vault it could not
// add to.
//
// The clipboard is offered rather than applied. Reading it is cheap but
// *acting* on it is not: iOS shows a system banner when an app reads the
// pasteboard, and silently prefilling a field with whatever the user last
// copied is startling when it turns out to be something private. So the
// clipboard is read once when the sheet opens, and only when it holds
// something that is already a link.

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

import '../src/rust/api/vault_api.dart';
import '../theme/websave_theme.dart';
import 'saves_controller.dart';

Future<void> showAddSaveSheet(
  BuildContext context,
  SavesController controller,
) => showDialog<void>(
  context: context,
  builder: (_) => _AddSaveDialog(controller: controller),
);

class _AddSaveDialog extends StatefulWidget {
  const _AddSaveDialog({required this.controller});
  final SavesController controller;

  @override
  State<_AddSaveDialog> createState() => _AddSaveDialogState();
}

class _AddSaveDialogState extends State<_AddSaveDialog> {
  final _field = TextEditingController();
  String? _clipboard;
  String? _error;
  bool _busy = false;

  @override
  void initState() {
    super.initState();
    _offerClipboard();
  }

  @override
  void dispose() {
    _field.dispose();
    super.dispose();
  }

  /// Read the clipboard once, and only surface it if it is already a link.
  Future<void> _offerClipboard() async {
    try {
      final data = await Clipboard.getData(Clipboard.kTextPlain);
      final text = data?.text?.trim();
      if (text == null || text.isEmpty) return;
      if (!await looksLikeUrl(text: text)) return;
      if (mounted) setState(() => _clipboard = text);
    } catch (_) {
      // A clipboard we cannot read is not worth reporting: the field works.
    }
  }

  Future<void> _add(String url) async {
    setState(() {
      _busy = true;
      _error = null;
    });
    try {
      await widget.controller.addByUrl(url);
      if (mounted) Navigator.of(context).pop();
    } catch (e) {
      // Rust returns the readable half after the exception name.
      final message = '$e'.replaceFirst(RegExp(r'^[A-Za-z]+Exception:?\s*'), '');
      if (mounted) {
        setState(() {
          _error = message;
          _busy = false;
        });
      }
    }
  }

  @override
  Widget build(BuildContext context) {
    final ws = context.ws;
    return AlertDialog(
      backgroundColor: ws.panel,
      shape: RoundedRectangleBorder(
        borderRadius: BorderRadius.circular(ws.cardRadius),
      ),
      title: Text(
        'Add a link',
        style: TextStyle(color: ws.text, fontSize: 16),
      ),
      content: SizedBox(
        width: 420,
        child: Column(
          mainAxisSize: MainAxisSize.min,
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            TextField(
              controller: _field,
              autofocus: true,
              enabled: !_busy,
              style: TextStyle(color: ws.text, fontSize: 13.5),
              decoration: InputDecoration(
                hintText: 'https://example.com',
                hintStyle: TextStyle(color: ws.textDim),
                border: OutlineInputBorder(
                  borderRadius: BorderRadius.circular(ws.controlRadius),
                ),
              ),
              onSubmitted: _busy ? null : _add,
            ),
            if (_clipboard case final pasted?) ...[
              const SizedBox(height: 10),
              _ClipboardChip(
                url: pasted,
                onTap: _busy ? null : () => _add(pasted),
              ),
            ],
            if (_error case final message?) ...[
              const SizedBox(height: 10),
              Text(
                message,
                style: TextStyle(color: ws.danger, fontSize: 12.5),
              ),
            ],
          ],
        ),
      ),
      actions: [
        TextButton(
          onPressed: _busy ? null : () => Navigator.of(context).pop(),
          child: const Text('Cancel'),
        ),
        FilledButton(
          onPressed: _busy ? null : () => _add(_field.text),
          style: ButtonStyle(
            backgroundColor: WidgetStatePropertyAll(ws.accent),
            foregroundColor: WidgetStatePropertyAll(ws.accentText),
          ),
          child: Text(_busy ? 'Saving…' : 'Save'),
        ),
      ],
    );
  }
}

/// The clipboard suggestion. Shows the link so it is obvious what would be
/// saved before it is saved.
class _ClipboardChip extends StatelessWidget {
  const _ClipboardChip({required this.url, required this.onTap});

  final String url;
  final VoidCallback? onTap;

  @override
  Widget build(BuildContext context) {
    final ws = context.ws;
    return InkWell(
      onTap: onTap,
      borderRadius: BorderRadius.circular(ws.controlRadius),
      child: Container(
        padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 8),
        decoration: BoxDecoration(
          color: ws.states.selected,
          border: Border.all(color: ws.accent.withValues(alpha: 0.5)),
          borderRadius: BorderRadius.circular(ws.controlRadius),
        ),
        child: Row(
          children: [
            Icon(Icons.content_paste, size: 15, color: ws.accent),
            const SizedBox(width: 8),
            Expanded(
              child: Text(
                'Paste & Add  $url',
                maxLines: 1,
                overflow: TextOverflow.ellipsis,
                style: TextStyle(color: ws.text, fontSize: 12.5),
              ),
            ),
          ],
        ),
      ),
    );
  }
}
