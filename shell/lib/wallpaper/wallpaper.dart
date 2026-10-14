// The desktop wallpaper (background layer). Default: the Silica wallpaper of silica-brand
// (exports/wallpapers, a trademark asset of the project), rendered in code from its recipe at
// any size, light or dark with the appearance. Alternatively a
// solid color or a user image, from ~/.config/silica/wallpaper.json: {"color": "#7a7b80"}
// or {"image": "/path"}.

import 'dart:convert';
import 'dart:io';
import 'dart:ui' as ui;

import 'package:flutter/widgets.dart';

import '../system/shell_channel.dart';

/// The wallpaper choice.
sealed class WallpaperChoice {
  const WallpaperChoice();
}

class GeneratedWallpaper extends WallpaperChoice {
  const GeneratedWallpaper();
}

class ColorWallpaper extends WallpaperChoice {
  const ColorWallpaper(this.color);
  final Color color;
}

class ImageWallpaper extends WallpaperChoice {
  const ImageWallpaper(this.path);
  final String path;
}

/// `#rrggbb` as an opaque color, or null.
Color? parseHexColor(String text) {
  final hex = text.startsWith('#') ? text.substring(1) : text;
  if (hex.length != 6) return null;
  final value = int.tryParse(hex, radix: 16);
  return value == null ? null : Color(0xFF000000 | value);
}

/// The choice from the JSON configuration; the default if missing or invalid.
WallpaperChoice parseWallpaperChoice(String? json) {
  if (json == null) return const GeneratedWallpaper();
  try {
    final value = jsonDecode(json);
    if (value is Map) {
      if (value['image'] case final String path when File(path).existsSync()) {
        return ImageWallpaper(path);
      }
      if (value['color'] case final String color) {
        if (parseHexColor(color) case final parsed?)
          return ColorWallpaper(parsed);
      }
    }
  } on FormatException {
    // Broken configuration: the default.
  }
  return const GeneratedWallpaper();
}

WallpaperChoice loadWallpaperChoice({Map<String, String>? env}) {
  final home = (env ?? Platform.environment)['HOME'] ?? '';
  final file = File('$home/.config/silica/wallpaper.json');
  return parseWallpaperChoice(
    file.existsSync() ? file.readAsStringSync() : null,
  );
}

class Wallpaper extends StatelessWidget {
  const Wallpaper({super.key, required this.choice});

  final WallpaperChoice choice;

  @override
  Widget build(BuildContext context) => GestureDetector(
    // A right click on the desktop: the menu bar shows the context menu (on top of the
    // windows, which the wallpaper is below).
    onSecondaryTapUp: (details) => ShellChannel.instance.broadcast(
      'desktopmenu',
      '${details.globalPosition.dx} ${details.globalPosition.dy}',
    ),
    child: _picture(),
  );

  Widget _picture() => switch (choice) {
    GeneratedWallpaper() => Builder(
      builder: (context) => CustomPaint(
        painter: SilicaWallpaperPainter(
          dark: MediaQuery.platformBrightnessOf(context) == Brightness.dark,
        ),
        size: Size.infinite,
      ),
    ),
    ColorWallpaper(:final color) => ColoredBox(color: color),
    ImageWallpaper(:final path) => Image.file(File(path), fit: BoxFit.cover),
  };
}

/// The Silica wallpaper (silica-brand `exports/wallpapers/wallpapers.json`): a diagonal
/// gradient from the top left to the bottom right and four soft elliptical glows, frames given
/// on a 2560 × 1440 canvas and scaled to cover the screen.
class SilicaWallpaperPainter extends CustomPainter {
  const SilicaWallpaperPainter({required this.dark});

  final bool dark;

  static const _lightBase = [
    (0.0, Color(0xFF8EC5FF)),
    (0.5, Color(0xFFF9C6D9)),
    (1.0, Color(0xFFFFE2B0)),
  ];
  static const _darkBase = [
    (0.0, Color(0xFF0B1030)),
    (0.55, Color(0xFF2A1446)),
    (1.0, Color(0xFF0A2A2E)),
  ];
  static const _frames = [
    Rect.fromLTWH(-300, 420, 1500, 1000),
    Rect.fromLTWH(1400, -380, 1500, 1100),
    Rect.fromLTWH(800, 760, 1300, 900),
    Rect.fromLTWH(1900, 900, 1000, 800),
  ];
  static const _lightGlows = [
    Color(0xCC6FE0B0),
    Color(0xB3A48BFF),
    Color(0xB3FFB07A),
    Color(0xB37CC4FF),
  ];
  static const _darkGlows = [
    Color(0xCC1F8A6E),
    Color(0xCC5B3FD0),
    Color(0x99C2456A),
    Color(0x991A5FB0),
  ];

  /// The recipe's "blur 220 px" at 2560 pt wide, as a Gaussian sigma: 235 reproduces the
  /// brand's PNG exports within half a level per channel on average (measured at 640 × 360).
  static const double blurSigma = 235;

  @override
  void paint(Canvas canvas, Size size) {
    final base = dark ? _darkBase : _lightBase;
    final rect = Offset.zero & size;
    canvas.drawRect(
      rect,
      Paint()
        ..shader = ui.Gradient.linear(
          Offset.zero,
          size.bottomRight(Offset.zero),
          [for (final (_, color) in base) color],
          [for (final (stop, _) in base) stop],
        ),
    );
    // Cover: the 2560 × 1440 canvas scaled to fill, centred.
    final scale = size.width / 2560 > size.height / 1440
        ? size.width / 2560
        : size.height / 1440;
    final dx = (size.width - 2560 * scale) / 2;
    final dy = (size.height - 1440 * scale) / 2;
    canvas.save();
    canvas.clipRect(rect);
    canvas.translate(dx, dy);
    canvas.scale(scale);
    final glows = dark ? _darkGlows : _lightGlows;
    for (final (i, frame) in _frames.indexed) {
      canvas.drawOval(
        frame,
        Paint()
          ..color = glows[i]
          ..maskFilter = const MaskFilter.blur(BlurStyle.normal, blurSigma),
      );
    }
    canvas.restore();
  }

  @override
  bool shouldRepaint(SilicaWallpaperPainter old) => old.dark != dark;
}

/// Twilight over the dunes: a gradient sky with a low glow, three rows of soft dunes
/// darker and darker towards the foreground.
class DunesPainter extends CustomPainter {
  const DunesPainter();

  @override
  void paint(Canvas canvas, Size size) {
    final rect = Offset.zero & size;
    canvas.drawRect(
      rect,
      Paint()
        ..shader = ui.Gradient.linear(
          rect.topCenter,
          rect.bottomCenter,
          const [
            Color(0xFF1B2A4E),
            Color(0xFF4B5A8C),
            Color(0xFFC98F8A),
            Color(0xFFF2C28B),
          ],
          const [0, 0.38, 0.62, 0.78],
        ),
    );
    canvas.drawCircle(
      Offset(size.width * 0.68, size.height * 0.66),
      size.width * 0.32,
      Paint()
        ..shader = ui.Gradient.radial(
          Offset(size.width * 0.68, size.height * 0.66),
          size.width * 0.32,
          const [Color(0x88FFE2B0), Color(0x00FFE2B0)],
        ),
    );
    const dunes = [
      (0.70, 0.06, Color(0xFFB0716A), Color(0xFF8C5560)),
      (0.80, 0.05, Color(0xFF7E4A5C), Color(0xFF5C3550)),
      (0.90, 0.045, Color(0xFF4A2C47), Color(0xFF2B1A33)),
    ];
    for (final (index, (base, amplitude, light, dark)) in dunes.indexed) {
      final top = size.height * (base - amplitude);
      final path = Path()..moveTo(0, size.height * base);
      const steps = 64;
      for (var i = 0; i <= steps; i++) {
        final x = size.width * i / steps;
        final t = i / steps;
        final wave = ui.lerpDouble(
          0,
          1,
          (1 + _wave(t * (2.2 + index * 0.7) + index)) / 2,
        )!;
        path.lineTo(x, size.height * base - size.height * amplitude * wave);
      }
      path
        ..lineTo(size.width, size.height)
        ..lineTo(0, size.height)
        ..close();
      canvas.drawPath(
        path,
        Paint()
          ..shader = ui.Gradient.linear(
            Offset(0, top),
            Offset(0, size.height),
            [light, dark],
          ),
      );
    }
  }

  // A soft asymmetric wave (dunes, not sinusoids).
  static double _wave(double x) {
    final s = x - x.floorToDouble();
    final up = s < 0.65 ? s / 0.65 : (1 - s) / 0.35;
    return (up * up * (3 - 2 * up)) * 2 - 1;
  }

  @override
  bool shouldRepaint(DunesPainter old) => false;
}
