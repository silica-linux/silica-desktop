// The colors of the Control Center's content (modules, detail panels, HUD), all Amalfi
// tokens (tokens.json) read from the theme in effect: no hand-picked alphas. The glass under
// the content is Glass/Thick (modules and details) or Glass/Regular (HUD), which Ossido draws.

import 'package:flutter/widgets.dart';
import 'package:silica_ui/silica_ui.dart';

@immutable
class CcPalette {
  const CcPalette._(this.colors);

  final AmalfiColors colors;

  bool get light => colors.brightness == Brightness.light;

  /// Titles, glyphs: `Text/Primary`.
  Color get text => colors.textPrimary;

  /// Subtitles: `Text/Secondary`.
  Color get secondary => colors.textSecondary;

  /// Section labels, times, the hint: `Text/Tertiary`.
  Color get tertiary => colors.textTertiary;

  /// A slider's track and the progress track: `Surface/Sunken`.
  Color get track => colors.surfaceSunken;

  /// The filled part of a slider, an active disc, an active tile: `Accent/Amalfi`.
  Color get accent => colors.accentAmalfi;

  /// A glyph on the accent: `Text/On Accent`.
  Color get onAccent => colors.textOnAccent;

  /// A disc that is off: `Text/Tertiary` (the kit's Toggle Off and Item Rest).
  Color get discOff => colors.textTertiary;

  /// Separators inside the detail panels: `Line/Separator`.
  Color get separator => colors.lineSeparator;

  /// The hover lift of a module: `State/Hover Overlay`.
  Color get hoverLift => colors.stateHoverOverlay;

  /// The band of the Focus options: `Surface/Sunken`.
  Color get band => colors.surfaceSunken;

  static CcPalette of(BuildContext context) =>
      CcPalette._(AmalfiTheme.of(context).colors);

  /// The text style color the content inherits.
  TextStyle get textStyle => TextStyle(color: text);
}
