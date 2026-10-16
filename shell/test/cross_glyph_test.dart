// The × of the small round buttons is centred in its circle (#166).

import 'dart:typed_data';
import 'dart:ui' as ui;

import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:silica_shell/system/cross_glyph.dart';

void main() {
  for (final size in [16.0, 18.0, 20.0, 22.0, 24.0]) {
    test('the cross is centred in a circle of $size', () async {
      const scale = 8.0;
      final recorder = ui.PictureRecorder();
      final canvas = Canvas(recorder)..scale(scale);
      CrossGlyph(
        const Color(0xFF000000),
        arm: size * 0.2,
        stroke: 1.5,
      ).paint(canvas, Size.square(size));
      final image = await recorder.endRecording().toImage(
        (size * scale).round(),
        (size * scale).round(),
      );
      final ByteData data = (await image.toByteData())!;
      var sx = 0.0, sy = 0.0, sum = 0.0;
      for (var y = 0; y < image.height; y++) {
        for (var x = 0; x < image.width; x++) {
          final a = data.getUint8((y * image.width + x) * 4 + 3) / 255;
          sx += (x + 0.5) * a;
          sy += (y + 0.5) * a;
          sum += a;
        }
      }
      expect(sx / sum / scale, closeTo(size / 2, 0.02));
      expect(sy / sum / scale, closeTo(size / 2, 0.02));
    });
  }
}
