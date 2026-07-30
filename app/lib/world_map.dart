// Where the pages in the vault are served from.
//
// No tile provider and no network: an equirectangular projection over a
// coarse coastline outline. That keeps the map as local-first as the rest of
// the app, renders instantly, and themes cleanly — a Mapbox tile would look
// like someone else's product pasted into this one.

import 'dart:math' as math;

import 'package:flutter/gestures.dart';
import 'package:flutter/material.dart';

import 'src/rust/api/vault_api.dart';
import 'world_outline.dart';

/// Equirectangular: longitude maps linearly to x, latitude to y.
///
/// The projection every "dots on a world map" wants. It distorts area badly
/// near the poles, which matters for a choropleth and not at all for this.
Offset project(double lat, double lon, Size size) => Offset(
  (lon + 180) / 360 * size.width,
  (90 - lat) / 180 * size.height,
);

class WorldMapScreen extends StatefulWidget {
  const WorldMapScreen({super.key});

  @override
  State<WorldMapScreen> createState() => _WorldMapScreenState();
}

class _WorldMapScreenState extends State<WorldMapScreen> {
  late Future<MapData> _data;
  MapPoint? _hovered;

  @override
  void initState() {
    super.initState();
    _data = mapData();
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(title: const Text('Served from')),
      body: FutureBuilder<MapData>(
        future: _data,
        builder: (context, snapshot) {
          if (snapshot.hasError) {
            return Center(child: Text('${snapshot.error}'));
          }
          if (!snapshot.hasData) {
            return const Center(child: CircularProgressIndicator());
          }
          final data = snapshot.data!;
          if (data.database == null) {
            return const _NoDatabase();
          }
          return Column(
            children: [
              Expanded(
                child: LayoutBuilder(
                  builder: (context, constraints) {
                    final size = _mapSize(constraints.biggest);
                    return Center(
                      child: MouseRegion(
                        onHover: (event) => _updateHover(event, size, data),
                        onExit: (_) => setState(() => _hovered = null),
                        child: CustomPaint(
                          size: size,
                          painter: _MapPainter(
                            points: data.points,
                            hovered: _hovered,
                            colors: Theme.of(context).colorScheme,
                          ),
                        ),
                      ),
                    );
                  },
                ),
              ),
              _Legend(data: data, hovered: _hovered),
            ],
          );
        },
      ),
    );
  }

  /// A 2:1 box, the natural aspect of an equirectangular world.
  Size _mapSize(Size available) {
    final width = math.min(available.width - 32, (available.height - 32) * 2);
    return Size(width, width / 2);
  }

  void _updateHover(PointerHoverEvent event, Size size, MapData data) {
    MapPoint? nearest;
    var best = 18.0;
    for (final point in data.points) {
      final distance =
          (project(point.latitude, point.longitude, size) - event.localPosition)
              .distance;
      if (distance < best) {
        best = distance;
        nearest = point;
      }
    }
    if (nearest != _hovered) setState(() => _hovered = nearest);
  }
}

class _MapPainter extends CustomPainter {
  _MapPainter({
    required this.points,
    required this.hovered,
    required this.colors,
  });

  final List<MapPoint> points;
  final MapPoint? hovered;
  final ColorScheme colors;

  @override
  void paint(Canvas canvas, Size size) {
    final land = Paint()
      ..style = PaintingStyle.fill
      ..color = colors.surfaceContainerHighest;
    for (final polygon in worldOutline) {
      final path = Path();
      for (var i = 0; i < polygon.length; i += 2) {
        final at = project(polygon[i + 1], polygon[i], size);
        i == 0 ? path.moveTo(at.dx, at.dy) : path.lineTo(at.dx, at.dy);
      }
      canvas.drawPath(path..close(), land);
    }

    // Area, not radius, tracks the save count — a host with 22 saves should
    // look four or five times a single-save host, not twenty-two times.
    final busiest = points.fold<int>(
      1,
      (max, p) => math.max(max, p.saveCount.toInt()),
    );
    for (final point in points) {
      final scale = math.sqrt(point.saveCount / busiest);
      final radius = 3.0 + scale * 9.0;
      final at = project(point.latitude, point.longitude, size);
      canvas.drawCircle(
        at,
        radius,
        Paint()..color = colors.primary.withValues(alpha: 0.55),
      );
      canvas.drawCircle(
        at,
        radius,
        Paint()
          ..style = PaintingStyle.stroke
          ..strokeWidth = 1.2
          ..color = colors.primary,
      );
    }

    if (hovered != null) {
      canvas.drawCircle(
        project(hovered!.latitude, hovered!.longitude, size),
        16,
        Paint()
          ..style = PaintingStyle.stroke
          ..strokeWidth = 2
          ..color = colors.tertiary,
      );
    }
  }

  @override
  bool shouldRepaint(_MapPainter old) =>
      old.hovered != hovered || old.points.length != points.length;
}

class _Legend extends StatelessWidget {
  const _Legend({required this.data, required this.hovered});
  final MapData data;
  final MapPoint? hovered;

  @override
  Widget build(BuildContext context) {
    final top = data.countries.take(6);
    return Container(
      width: double.infinity,
      padding: const EdgeInsets.all(16),
      decoration: BoxDecoration(
        border: Border(
          top: BorderSide(color: Theme.of(context).dividerColor),
        ),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Text(
            hovered == null
                ? '${data.points.length} hosts plotted · ${data.locatedHosts} located'
                : '${hovered!.host} — ${hovered!.saveCount} save(s) · '
                      '${hovered!.provider.isEmpty ? hovered!.ip : hovered!.provider}'
                      '${hovered!.anycast ? " (anycast)" : ""} · '
                      '${hovered!.city.isEmpty ? "?" : hovered!.city}, ${hovered!.country}',
            style: Theme.of(context).textTheme.titleSmall,
          ),
          const SizedBox(height: 8),
          // The provider list is the honest one, so it leads. Countries stay
          // because they are still meaningful for self-hosted sites.
          if (data.providers.isNotEmpty)
            Wrap(
              spacing: 14,
              children: [
                for (final (org, count) in data.providers.take(5))
                  Text('$org $count'),
              ],
            ),
          Wrap(
            spacing: 14,
            children: [
              for (final (code, count) in top)
                Text(
                  '$code $count',
                  style: Theme.of(context).textTheme.bodySmall,
                ),
            ],
          ),
          const SizedBox(height: 8),
          Text(
            data.providers.isEmpty
                ? 'Where the bytes come from, not where the site is from. '
                      'Most of these are CDN edges. Install an ASN database to '
                      'see which network actually serves each host.'
                : 'Where the bytes come from, not where the site is from. For '
                      'the networks marked anycast the dot is roughly '
                      'arbitrary — the provider name is the real answer.',
            style: Theme.of(context).textTheme.bodySmall?.copyWith(
              color: Theme.of(context).hintColor,
            ),
          ),
          Text(
            data.database ?? '',
            style: Theme.of(context).textTheme.bodySmall?.copyWith(
              color: Theme.of(context).hintColor,
            ),
          ),
        ],
      ),
    );
  }
}

class _NoDatabase extends StatelessWidget {
  const _NoDatabase();

  @override
  Widget build(BuildContext context) {
    return Center(
      child: ConstrainedBox(
        constraints: const BoxConstraints(maxWidth: 460),
        child: Column(
          mainAxisAlignment: MainAxisAlignment.center,
          children: [
            const Icon(Icons.public_off, size: 48),
            const SizedBox(height: 16),
            Text(
              'No geolocation database',
              style: Theme.of(context).textTheme.titleMedium,
            ),
            const SizedBox(height: 8),
            Text(
              'The map needs an offline IP database so no part of your '
              'browsing is sent to a geolocation service. Download '
              'dbip-city-lite (CC BY 4.0) and save it into the vault\'s '
              'assets folder as geoip.mmdb.',
              textAlign: TextAlign.center,
              style: Theme.of(context).textTheme.bodyMedium,
            ),
          ],
        ),
      ),
    );
  }
}
