// The appearance-aware colors of the Control Center's content (modules, detail panels,
// HUDs). The glass under them is Amalfi's Thick/Regular, near-white in light, so the content
// follows the appearance: dark text and glyphs in light, today's white in dark.

import 'package:flutter/widgets.dart';

@immutable
class CcPalette {
  const CcPalette({
    required this.light,
    required this.text,
    required this.secondary,
    required this.trackRest,
    required this.trackFill,
    required this.separator,
    required this.switchOff,
    required this.hudDot,
    required this.outputsFill,
    required this.hoverLift,
    required this.nowPlayingTrack,
  });

  final bool light;

  /// Titles, glyphs and slider fills on the glass.
  final Color text;

  /// Subtitles, section labels, times.
  final Color secondary;
  final Color trackRest;
  final Color trackFill;
  final Color separator;
  final Color switchOff;
  final Color hudDot;
  final Color outputsFill;
  final Color hoverLift;
  final Color nowPlayingTrack;

  static const CcPalette dark = CcPalette(
    light: false,
    text: Color(0xFFFFFFFF),
    secondary: Color(0xFFB7B8BE),
    trackRest: Color(0xFF7E7F84),
    trackFill: Color(0xFFFFFFFF),
    separator: Color(0x1AFFFFFF),
    switchOff: Color(0x40FFFFFF),
    hudDot: Color(0x33FFFFFF),
    outputsFill: Color(0x3DFFFFFF),
    hoverLift: Color(0x0CFFFFFF),
    nowPlayingTrack: Color(0xFF232323),
  );

  static const CcPalette lightMode = CcPalette(
    light: true,
    // Amalfi tokens.json, light: Text/Primary.
    text: Color(0xFF1D1E21),
    // Amalfi tokens.json, light: Text/Secondary.
    secondary: Color(0xFF5F636B),
    trackRest: Color(0x2E000000),
    trackFill: Color(0xFF1D1E21),
    separator: Color(0x1A000000),
    switchOff: Color(0x33000000),
    hudDot: Color(0x33000000),
    outputsFill: Color(0x1A000000),
    hoverLift: Color(0x0C000000),
    nowPlayingTrack: Color(0x26000000),
  );

  static CcPalette of(BuildContext context) =>
      MediaQuery.maybePlatformBrightnessOf(context) == Brightness.dark
      ? dark
      : lightMode;

  /// The text style color the content inherits.
  TextStyle get textStyle => TextStyle(color: text);
}
