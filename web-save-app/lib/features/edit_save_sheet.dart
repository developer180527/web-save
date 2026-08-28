// Editing a save's title, notes and tags.
//
// One sheet for all three fields rather than three inline editors: the React
// app's edit panel worked the same way, and a single Save button is far
// easier to reason about than three fields each racing their own write.
//
// Only changed fields are sent. Core's SavePatch treats null as "leave
// alone", so an untouched notes field cannot clobber notes written elsewhere
// -- by the extension, or by another window on the same vault.

import 'package:flutter/material.dart';

import '../src/rust/api/vault_api.dart';
import '../theme/websave_theme.dart';
import 'saves_controller.dart';

Future<void> showEditSheet(
  BuildContext context,
  SaveView save,
  SavesController controller,
) {
  return showDialog<void>(
    context: context,
    builder: (_) => _EditDialog(save: save, controller: controller),
  );
}

class _EditDialog extends StatefulWidget {
  const _EditDialog({required this.save, required this.controller});

  final SaveView save;
  final SavesController controller;

  @override
  State<_EditDialog> createState() => _EditDialogState();
}

class _EditDialogState extends State<_EditDialog> {
  late final _title = TextEditingController(text: widget.save.title);
  late final _description = TextEditingController(
    text: widget.save.description,
  );
  late final _tags = TextEditingController(text: widget.save.tags.join(', '));
  bool _saving = false;

  @override
  void dispose() {
    _title.dispose();
    _description.dispose();
    _tags.dispose();
    super.dispose();
  }

  /// Comma or space separated, deduped case-insensitively — the same shape
  /// core normalises to, so what the user typed and what comes back agree.
  List<String> get _parsedTags {
    final seen = <String>{};
    final out = <String>[];
    for (final raw in _tags.text.split(RegExp(r'[,\s]+'))) {
      final tag = raw.trim();
      if (tag.isEmpty) continue;
      if (seen.add(tag.toLowerCase())) out.add(tag);
    }
    return out;
  }

  Future<void> _save() async {
    setState(() => _saving = true);
    final save = widget.save;
    final controller = widget.controller;

    // Only what actually changed. Sending an unchanged field would overwrite
    // whatever another writer put there since this dialog opened.
    final titleChanged = _title.text != save.title;
    final descriptionChanged = _description.text != save.description;
    if (titleChanged || descriptionChanged) {
      await controller.edit(
        save,
        title: titleChanged ? _title.text : null,
        description: descriptionChanged ? _description.text : null,
      );
    }
    final tags = _parsedTags;
    if (!_sameTags(tags, save.tags)) {
      await controller.setTagsFor(save, tags);
    }
    if (mounted) Navigator.pop(context);
  }

  static bool _sameTags(List<String> a, List<String> b) {
    if (a.length != b.length) return false;
    for (var i = 0; i < a.length; i++) {
      if (a[i].toLowerCase() != b[i].toLowerCase()) return false;
    }
    return true;
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
        'Edit save',
        style: TextStyle(
          color: ws.text,
          fontSize: 15,
          fontWeight: FontWeight.w700,
        ),
      ),
      content: SizedBox(
        width: 420,
        child: Column(
          mainAxisSize: MainAxisSize.min,
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            _Field(label: 'Title', controller: _title, autofocus: true),
            const SizedBox(height: 12),
            _Field(label: 'Notes', controller: _description, maxLines: 4),
            const SizedBox(height: 12),
            _Field(
              label: 'Tags',
              controller: _tags,
              hint: 'comma or space separated',
            ),
          ],
        ),
      ),
      actions: [
        TextButton(
          onPressed: _saving ? null : () => Navigator.pop(context),
          child: Text('Cancel', style: TextStyle(color: ws.textDim)),
        ),
        FilledButton(
          onPressed: _saving ? null : _save,
          style: FilledButton.styleFrom(
            backgroundColor: ws.accent,
            foregroundColor: ws.accentText,
          ),
          child: Text(_saving ? 'Saving…' : 'Save'),
        ),
      ],
    );
  }
}

class _Field extends StatelessWidget {
  const _Field({
    required this.label,
    required this.controller,
    this.maxLines = 1,
    this.hint,
    this.autofocus = false,
  });

  final String label;
  final TextEditingController controller;
  final int maxLines;
  final String? hint;
  final bool autofocus;

  @override
  Widget build(BuildContext context) {
    final ws = context.ws;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Text(
          label,
          style: TextStyle(
            color: ws.textDim,
            fontSize: 11.5,
            fontWeight: FontWeight.w600,
          ),
        ),
        const SizedBox(height: 4),
        TextField(
          controller: controller,
          maxLines: maxLines,
          autofocus: autofocus,
          style: TextStyle(color: ws.text, fontSize: 13.5),
          decoration: InputDecoration(
            hintText: hint,
            hintStyle: TextStyle(color: ws.textDim, fontSize: 12.5),
          ),
        ),
      ],
    );
  }
}
