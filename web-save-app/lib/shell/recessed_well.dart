// The surface content is kept in.
//
// Flutter has no inset shadow: BoxShadow only ever falls outside the box. So
// the lip is drawn as two gradient strips overlaid on the well's own edges —
// dark at the boundary, fading to nothing over `lipDepth`. Cheap, static, and
// no blur filter involved, which matters because this sits under a scrolling
// grid that is already painting a shadow per card.
//
// Only the top and leading edges are shaded. A real recess would darken all
// four, but the bottom and trailing edges of this one run off the window
// rather than meeting a lip, and shading an edge that has no wall above it is
// what makes these effects read as a vignette instead of a depth cue.

import 'package:flutter/material.dart';

import '../theme/websave_theme.dart';

class RecessedWell extends StatelessWidget {
  const RecessedWell({
    super.key,
    required this.child,
    this.shadeLeading = false,
  });

  final Widget child;

  /// Whether the leading edge meets chrome too.
  ///
  /// True beside a sidebar, false when the well runs to the window edge —
  /// where there is nothing casting into it.
  final bool shadeLeading;

  @override
  Widget build(BuildContext context) {
    final elevation = context.ws.elevation;
    return DecoratedBox(
      decoration: BoxDecoration(color: elevation.well),
      child: Stack(
        children: [
          Positioned.fill(child: child),
          // Painted over the content rather than under it: the lip is the
          // near edge of the recess, so things inside the well pass beneath
          // it as they scroll up and out.
          Positioned(
            top: 0,
            left: 0,
            right: 0,
            height: elevation.lipDepth,
            child: IgnorePointer(
              child: DecoratedBox(
                decoration: BoxDecoration(
                  gradient: LinearGradient(
                    begin: Alignment.topCenter,
                    end: Alignment.bottomCenter,
                    colors: [elevation.wellShade, elevation.wellShade.withAlpha(0)],
                  ),
                ),
              ),
            ),
          ),
          if (shadeLeading)
            Positioned(
              top: 0,
              bottom: 0,
              left: 0,
              // Shallower than the top: the light comes from above, so a
              // side wall catches a narrower band of shadow than a lip
              // directly under the source.
              width: elevation.lipDepth * 0.6,
              child: IgnorePointer(
                child: DecoratedBox(
                  decoration: BoxDecoration(
                    gradient: LinearGradient(
                      begin: Alignment.centerLeft,
                      end: Alignment.centerRight,
                      colors: [
                        elevation.wellShade,
                        elevation.wellShade.withAlpha(0),
                      ],
                    ),
                  ),
                ),
              ),
            ),
        ],
      ),
    );
  }
}
