// The activity log, as seen from Settings.
//
// This exists mainly for work the app cannot otherwise account for. The iOS
// background sweep runs with no Flutter engine, on a schedule the OS decides,
// usually while the phone is in a pocket — os_log would show it in Console.app
// on a tethered Mac, which is no help to someone asking why their links
// stopped being checked. The log lives in the vault, so whichever process did
// the work is the one that records it.

import 'package:flutter/material.dart';

import '../src/rust/api/vault_api.dart';
import '../theme/websave_theme.dart';

class ActivityLogPanel extends StatefulWidget {
  const ActivityLogPanel({super.key});

  @override
  State<ActivityLogPanel> createState() => _ActivityLogPanelState();
}

class _ActivityLogPanelState extends State<ActivityLogPanel> {
  List<ActivityEntryView>? _entries;
  String? _error;

  @override
  void initState() {
    super.initState();
    _reload();
  }

  Future<void> _reload() async {
    try {
      final entries = await recentActivity(limit: 100);
      if (mounted) {
        setState(() {
          _entries = entries;
          _error = null;
        });
      }
    } catch (e) {
      if (mounted) setState(() => _error = '$e');
    }
  }

  Future<void> _clear() async {
    try {
      await clearActivity();
      await _reload();
    } catch (e) {
      if (mounted) setState(() => _error = '$e');
    }
  }

  String _when(int seconds) {
    final at = DateTime.fromMillisecondsSinceEpoch(seconds * 1000);
    final gap = DateTime.now().difference(at);
    if (gap.inMinutes < 1) return 'just now';
    if (gap.inHours < 1) return '${gap.inMinutes} min ago';
    if (gap.inDays < 1) return '${gap.inHours} h ago';
    return '${gap.inDays} d ago';
  }

  Color _levelColor(String level, WebSaveTheme ws) => switch (level) {
    'error' => ws.health.dead,
    'warn' => ws.health.changed,
    _ => ws.health.active,
  };

  @override
  Widget build(BuildContext context) {
    final ws = context.ws;
    if (_error case final message?) {
      return Text(message, style: TextStyle(color: ws.danger, fontSize: 12.5));
    }
    final entries = _entries;
    if (entries == null) {
      return const SizedBox(
        height: 40,
        child: Center(child: CircularProgressIndicator(strokeWidth: 2)),
      );
    }

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Container(
          // Bounded and scrolled internally: Settings is already a ListView,
          // and a hundred rows spliced into it would bury every section below.
          constraints: const BoxConstraints(maxHeight: 320),
          decoration: BoxDecoration(
            border: Border.all(color: ws.border),
            borderRadius: BorderRadius.circular(ws.cardRadius),
          ),
          child: entries.isEmpty
              ? Padding(
                  padding: EdgeInsets.all(ws.cardPadding),
                  child: Text(
                    'Nothing recorded yet. Background checks write here when '
                    'they run.',
                    style: TextStyle(color: ws.textDim, fontSize: 12.5),
                  ),
                )
              : ListView.separated(
                  shrinkWrap: true,
                  padding: const EdgeInsets.symmetric(vertical: 4),
                  itemCount: entries.length,
                  separatorBuilder: (_, _) =>
                      Divider(color: ws.border, height: 1),
                  itemBuilder: (context, i) {
                    final entry = entries[i];
                    return Padding(
                      padding: const EdgeInsets.symmetric(
                        horizontal: 12,
                        vertical: 8,
                      ),
                      child: Row(
                        crossAxisAlignment: CrossAxisAlignment.start,
                        children: [
                          Padding(
                            padding: const EdgeInsets.only(top: 5),
                            child: Container(
                              width: 7,
                              height: 7,
                              decoration: BoxDecoration(
                                color: _levelColor(entry.level, ws),
                                shape: BoxShape.circle,
                              ),
                            ),
                          ),
                          const SizedBox(width: 10),
                          Expanded(
                            child: Column(
                              crossAxisAlignment: CrossAxisAlignment.start,
                              children: [
                                Text(
                                  entry.message,
                                  style: TextStyle(
                                    color: ws.text,
                                    fontSize: 12.5,
                                    height: 1.35,
                                  ),
                                ),
                                Text(
                                  '${entry.source} · ${_when(entry.at)}',
                                  style: TextStyle(
                                    color: ws.textDim,
                                    fontSize: 11.5,
                                  ),
                                ),
                              ],
                            ),
                          ),
                        ],
                      ),
                    );
                  },
                ),
        ),
        const SizedBox(height: 8),
        Row(
          children: [
            TextButton.icon(
              onPressed: _reload,
              icon: const Icon(Icons.refresh, size: 15),
              label: const Text('Refresh'),
            ),
            const Spacer(),
            TextButton(
              onPressed: entries.isEmpty ? null : _clear,
              child: Text('Clear', style: TextStyle(color: ws.textDim)),
            ),
          ],
        ),
      ],
    );
  }
}
