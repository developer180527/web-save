// WebSave's design vocabulary, as a flutter_theme_engine ThemeModel.
//
// A direct port of the CSS custom properties in src/App.css. Everything
// Material understands is projected onto ThemeData by toThemeData(); the rest
// -- the five link-health colours, hover and selection surfaces, the metrics
// -- has no Material equivalent and is read straight off the model.
//
// The rule this file exists to enforce: no widget in the app hardcodes a
// colour or a radius. If a pack cannot restate it, it is not a token yet.

import 'package:flutter/foundation.dart' show defaultTargetPlatform;
import 'package:flutter/material.dart';
import 'package:flutter_theme_engine/flutter_theme_engine.dart';

import 'link_health_colors.dart';

@immutable
class WebSaveTheme extends ThemeModel {
  const WebSaveTheme({
    required Brightness brightness,
    required this.background,
    required this.panel,
    required this.border,
    required this.text,
    required this.textDim,
    required this.accent,
    required this.accentText,
    required this.danger,
    required this.health,
    required this.states,
    required this.swatch,
    this.cardRadius = 10,
    this.controlRadius = 8,
    this.iconTileRadius = 7,
    this.rowHeight = 36,
    this.iconTileSize = 30,
    this.cardPadding = 12,
    this.hitTarget = 28,
    this.fontFamily,
    this.baseFontSize = 14,
    this.translucentChrome = false,
    // The field is private, and Dart has no private named parameters, so an
    // initialising formal is not available here.
    // ignore: prefer_initializing_formals
  }) : _brightness = brightness;

  final Brightness _brightness;

  // ---- surfaces (--bg, --bg-panel, --border) ----
  final Color background;
  final Color panel;
  final Color border;

  // ---- text (--text, --text-dim) ----
  final Color text;
  final Color textDim;

  // ---- accent (--accent, --accent-text, --danger) ----
  final Color accent;
  final Color accentText;
  final Color danger;

  /// The five link states. See [LinkHealthColors].
  final LinkHealthColors health;

  /// Hover, pressed and selected surfaces.
  final InteractionColors states;

  /// Three colours for the theme picker.
  ///
  /// Stored rather than derived on purpose. `ColorScheme.fromSeed` runs the
  /// HCT quantiser, and a picker showing forty packs that each called
  /// `toThemeData()` would hitch visibly on first paint. The picker draws
  /// these and never touches `pack.themeData`.
  final List<Color> swatch;

  // ---- metrics ----
  final double cardRadius;
  final double controlRadius;
  final double iconTileRadius;

  /// List row height. The main density lever: desktop packs use 36, touch
  /// variants 52.
  final double rowHeight;
  final double iconTileSize;
  final double cardPadding;

  /// Minimum interactive size. 28 suits a mouse; a touch pack raises it to
  /// 44, which is why this is a token and not a constant.
  final double hitTarget;

  /// Typeface for the whole app, or null to use the platform's own.
  ///
  /// Null on purpose by default. Apple platforms have San Francisco, which is
  /// what every other app on the device is set in and what the Cupertino
  /// glyphs were drawn to sit beside; naming a face here overrides it. This
  /// used to be hardcoded to Avenir Next, which is present on Apple hardware
  /// but is not the system font, so the app read as slightly foreign
  /// everywhere without being obviously wrong anywhere.
  final String? fontFamily;
  final double baseFontSize;

  /// macOS vibrancy behind the sidebar and title bar.
  final bool translucentChrome;

  @override
  Brightness get brightness => _brightness;

  @override
  ThemeData toThemeData() {
    final isDark = _brightness == Brightness.dark;
    // Built explicitly rather than with fromSeed: the CSS already names every
    // surface, and letting Material derive them from a seed would quietly
    // change the palette the app has always had.
    final scheme = ColorScheme(
      brightness: _brightness,
      primary: accent,
      onPrimary: accentText,
      secondary: accent,
      onSecondary: accentText,
      error: danger,
      onError: accentText,
      surface: background,
      onSurface: text,
      surfaceContainer: panel,
      surfaceContainerHighest: states.hover,
      onSurfaceVariant: textDim,
      outline: border,
      outlineVariant: border,
      shadow: isDark ? Colors.black : const Color(0x33000000),
    );

    return ThemeData(
      useMaterial3: true,
      brightness: _brightness,
      colorScheme: scheme,
      scaffoldBackgroundColor: background,
      canvasColor: background,
      dividerColor: border,
      fontFamily: fontFamily,
      dividerTheme: DividerThemeData(color: border, space: 1, thickness: 1),
      cardTheme: CardThemeData(
        color: panel,
        elevation: 0,
        shape: RoundedRectangleBorder(
          borderRadius: BorderRadius.circular(cardRadius),
          side: BorderSide(color: border),
        ),
      ),
      inputDecorationTheme: InputDecorationTheme(
        filled: true,
        fillColor: panel,
        isDense: true,
        border: OutlineInputBorder(
          borderRadius: BorderRadius.circular(controlRadius),
          borderSide: BorderSide(color: border),
        ),
        enabledBorder: OutlineInputBorder(
          borderRadius: BorderRadius.circular(controlRadius),
          borderSide: BorderSide(color: border),
        ),
        focusedBorder: OutlineInputBorder(
          borderRadius: BorderRadius.circular(controlRadius),
          borderSide: BorderSide(color: accent),
        ),
      ),
      appBarTheme: AppBarTheme(
        backgroundColor: panel,
        foregroundColor: text,
        elevation: 0,
        scrolledUnderElevation: 0,
        centerTitle: false,
      ),
      listTileTheme: ListTileThemeData(
        selectedTileColor: states.selected,
        iconColor: textDim,
      ),
      // Typography follows the real platform rather than a fixed one. It used
      // to be pinned to macOS, which picked Apple's metrics for Android too.
      textTheme: Typography.material2021(platform: defaultTargetPlatform).black
          .apply(bodyColor: text, displayColor: text, fontFamily: fontFamily),
    );
  }

  /// Only needed for animated transitions between packs; the engine falls
  /// back to a hard switch without it.
  @override
  ThemeModel lerp(ThemeModel other, double t) {
    if (other is! WebSaveTheme) return t < 0.5 ? this : other;
    return copyWith(
      background: Color.lerp(background, other.background, t),
      panel: Color.lerp(panel, other.panel, t),
      border: Color.lerp(border, other.border, t),
      text: Color.lerp(text, other.text, t),
      textDim: Color.lerp(textDim, other.textDim, t),
      accent: Color.lerp(accent, other.accent, t),
      accentText: Color.lerp(accentText, other.accentText, t),
      danger: Color.lerp(danger, other.danger, t),
      health: LinkHealthColors.lerp(health, other.health, t),
      states: InteractionColors.lerp(states, other.states, t),
      cardRadius: lerpDouble(cardRadius, other.cardRadius, t),
      controlRadius: lerpDouble(controlRadius, other.controlRadius, t),
      iconTileRadius: lerpDouble(iconTileRadius, other.iconTileRadius, t),
      rowHeight: lerpDouble(rowHeight, other.rowHeight, t),
      iconTileSize: lerpDouble(iconTileSize, other.iconTileSize, t),
      cardPadding: lerpDouble(cardPadding, other.cardPadding, t),
      hitTarget: lerpDouble(hitTarget, other.hitTarget, t),
      brightness: t < 0.5 ? _brightness : other._brightness,
    );
  }

  static double lerpDouble(double a, double b, double t) => a + (b - a) * t;

  WebSaveTheme copyWith({
    Brightness? brightness,
    Color? background,
    Color? panel,
    Color? border,
    Color? text,
    Color? textDim,
    Color? accent,
    Color? accentText,
    Color? danger,
    LinkHealthColors? health,
    InteractionColors? states,
    List<Color>? swatch,
    double? cardRadius,
    double? controlRadius,
    double? iconTileRadius,
    double? rowHeight,
    double? iconTileSize,
    double? cardPadding,
    double? hitTarget,
    String? fontFamily,
    double? baseFontSize,
    bool? translucentChrome,
  }) => WebSaveTheme(
    brightness: brightness ?? _brightness,
    background: background ?? this.background,
    panel: panel ?? this.panel,
    border: border ?? this.border,
    text: text ?? this.text,
    textDim: textDim ?? this.textDim,
    accent: accent ?? this.accent,
    accentText: accentText ?? this.accentText,
    danger: danger ?? this.danger,
    health: health ?? this.health,
    states: states ?? this.states,
    swatch: swatch ?? this.swatch,
    cardRadius: cardRadius ?? this.cardRadius,
    controlRadius: controlRadius ?? this.controlRadius,
    iconTileRadius: iconTileRadius ?? this.iconTileRadius,
    rowHeight: rowHeight ?? this.rowHeight,
    iconTileSize: iconTileSize ?? this.iconTileSize,
    cardPadding: cardPadding ?? this.cardPadding,
    hitTarget: hitTarget ?? this.hitTarget,
    fontFamily: fontFamily ?? this.fontFamily,
    baseFontSize: baseFontSize ?? this.baseFontSize,
    translucentChrome: translucentChrome ?? this.translucentChrome,
  );

  /// The roomy variant, for touch. Colours are untouched — only the metrics
  /// that decide how fat a finger needs the UI to be.
  WebSaveTheme get touchVariant =>
      copyWith(rowHeight: 52, iconTileSize: 34, cardPadding: 16, hitTarget: 44);
}

/// Reads the active model. Establishes an InheritedWidget dependency, so a
/// widget using this rebuilds on theme change automatically.
extension WebSaveThemeContext on BuildContext {
  WebSaveTheme get ws => ThemeEngineScope.of<WebSaveTheme>(this).current.model;
}
