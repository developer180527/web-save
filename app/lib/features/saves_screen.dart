// The saves list.
//
// Deliberately token-only: no colour, radius or height in this file is a
// literal. That is the property that makes a pack able to restyle the app
// without touching a widget, and the thing to check when reviewing new UI.

import 'package:flutter/material.dart';

import '../shell/layout_scope.dart';
import '../src/rust/api/vault_api.dart';
import '../theme/websave_theme.dart';
import 'save_row.dart';

class SavesScreen extends StatefulWidget {
  const SavesScreen({super.key});

  @override
  State<SavesScreen> createState() => _SavesScreenState();
}

class _SavesScreenState extends State<SavesScreen> {
  String _query = '';
  int? _selectedId;
  late Future<(List<SaveView>, StatsView)> _data;

  @override
  void initState() {
    super.initState();
    _load();
  }

  void _load() {
    _data = Future.wait([
      listSaves(query: _query.isEmpty ? null : _query, limit: 500),
      vaultStats(),
    ]).then((r) => (r[0] as List<SaveView>, r[1] as StatsView));
  }

  @override
  Widget build(BuildContext context) {
    final ws = context.ws;
    return Column(
      children: [
        _SearchBar(
          onChanged: (value) => setState(() {
            _query = value;
            _load();
          }),
        ),
        Expanded(
          child: FutureBuilder<(List<SaveView>, StatsView)>(
            future: _data,
            builder: (context, snapshot) {
              if (snapshot.hasError) {
                return Center(
                  child: Text(
                    '${snapshot.error}',
                    style: TextStyle(color: ws.danger),
                  ),
                );
              }
              if (!snapshot.hasData) {
                return const Center(child: CircularProgressIndicator());
              }
              final (saves, stats) = snapshot.data!;
              return Column(
                children: [
                  _StatsStrip(stats: stats),
                  Expanded(
                    child: saves.isEmpty
                        ? Center(
                            child: Text(
                              'No saves matched.',
                              style: TextStyle(color: ws.textDim),
                            ),
                          )
                        : ListView.builder(
                            padding: EdgeInsets.symmetric(
                              horizontal: ws.cardPadding,
                              vertical: 6,
                            ),
                            itemCount: saves.length,
                            itemBuilder: (context, i) => SaveRow(
                              save: saves[i],
                              selected: saves[i].id == _selectedId,
                              onTap: () =>
                                  setState(() => _selectedId = saves[i].id),
                            ),
                          ),
                  ),
                ],
              );
            },
          ),
        ),
      ],
    );
  }
}

class _SearchBar extends StatelessWidget {
  const _SearchBar({required this.onChanged});
  final ValueChanged<String> onChanged;

  @override
  Widget build(BuildContext context) {
    final ws = context.ws;
    return Container(
      padding: EdgeInsets.fromLTRB(
        ws.cardPadding,
        ws.cardPadding,
        ws.cardPadding,
        8,
      ),
      decoration: BoxDecoration(
        color: ws.background,
        border: Border(bottom: BorderSide(color: ws.border)),
      ),
      child: SafeArea(
        bottom: false,
        child: SizedBox(
          height: ws.rowHeight,
          child: TextField(
            onChanged: onChanged,
            style: TextStyle(fontSize: 13.5, color: ws.text),
            decoration: InputDecoration(
              hintText: 'Search titles, notes, tags and archived text…',
              hintStyle: TextStyle(color: ws.textDim, fontSize: 13.5),
              prefixIcon: Icon(Icons.search, size: 17, color: ws.textDim),
              prefixIconConstraints: const BoxConstraints(minWidth: 34),
              contentPadding: const EdgeInsets.symmetric(vertical: 8),
            ),
          ),
        ),
      ),
    );
  }
}

class _StatsStrip extends StatelessWidget {
  const _StatsStrip({required this.stats});
  final StatsView stats;

  @override
  Widget build(BuildContext context) {
    final ws = context.ws;
    final compact = context.layout.isCompact;
    final entries = <(String, int, Color)>[
      ('active', stats.active.toInt(), ws.health.active),
      ('changed', stats.changed.toInt(), ws.health.changed),
      ('redirected', stats.redirected.toInt(), ws.health.redirected),
      ('dead', stats.dead.toInt(), ws.health.dead),
    ];
    return Container(
      width: double.infinity,
      padding: EdgeInsets.symmetric(horizontal: ws.cardPadding, vertical: 8),
      child: Wrap(
        spacing: 14,
        runSpacing: 4,
        crossAxisAlignment: WrapCrossAlignment.center,
        children: [
          Text(
            '${stats.total} saves',
            style: TextStyle(
              color: ws.text,
              fontSize: 12,
              fontWeight: FontWeight.w600,
            ),
          ),
          if (!compact)
            Text(
              '${stats.unread} unread',
              style: TextStyle(color: ws.textDim, fontSize: 12),
            ),
          for (final (label, count, color) in entries)
            Row(
              mainAxisSize: MainAxisSize.min,
              children: [
                Container(
                  width: 7,
                  height: 7,
                  decoration: BoxDecoration(
                    color: color,
                    shape: BoxShape.circle,
                  ),
                ),
                const SizedBox(width: 5),
                Text(
                  compact ? '$count' : '$count $label',
                  style: TextStyle(color: ws.textDim, fontSize: 12),
                ),
              ],
            ),
        ],
      ),
    );
  }
}
