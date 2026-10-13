// Lo sfondo del desktop (layer background). Predefinito: generato in codice, originale
// (niente immagini di Apple né asset da licenziare): un cielo al crepuscolo con dune di
// sabbia di silice. In alternativa un colore pieno o un'immagine dell'utente, da
// ~/.config/silica/wallpaper.json: {"color": "#7a7b80"} oppure {"image": "/percorso"}.

import 'dart:convert';
import 'dart:io';
import 'dart:ui' as ui;

import 'package:flutter/widgets.dart';

/// La scelta dello sfondo.
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

/// `#rrggbb` in un colore opaco, o null.
Color? parseHexColor(String text) {
  final hex = text.startsWith('#') ? text.substring(1) : text;
  if (hex.length != 6) return null;
  final value = int.tryParse(hex, radix: 16);
  return value == null ? null : Color(0xFF000000 | value);
}

/// La scelta dalla configurazione JSON; il predefinito se manca o non è valida.
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
    // Configurazione rovinata: il predefinito.
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
  Widget build(BuildContext context) => switch (choice) {
    GeneratedWallpaper() => const CustomPaint(
      painter: DunesPainter(),
      size: Size.infinite,
    ),
    ColorWallpaper(:final color) => ColoredBox(color: color),
    ImageWallpaper(:final path) => Image.file(File(path), fit: BoxFit.cover),
  };
}

/// Crepuscolo sulle dune: cielo a gradiente con un bagliore basso, tre file di dune
/// morbide sempre più scure verso il primo piano.
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

  // Un'onda morbida asimmetrica (dune, non sinusoidi).
  static double _wave(double x) {
    final s = x - x.floorToDouble();
    final up = s < 0.65 ? s / 0.65 : (1 - s) / 0.35;
    return (up * up * (3 - 2 * up)) * 2 - 1;
  }

  @override
  bool shouldRepaint(DunesPainter old) => false;
}
