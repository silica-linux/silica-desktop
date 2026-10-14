import 'dart:io';

import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:silica_shell/wallpaper/wallpaper.dart';

void main() {
  test('the wallpaper choice from the configuration', () {
    expect(parseWallpaperChoice(null), isA<GeneratedWallpaper>());
    expect(parseWallpaperChoice('non json'), isA<GeneratedWallpaper>());
    final color = parseWallpaperChoice('{"color": "#7a7b80"}');
    expect((color as ColorWallpaper).color, const Color(0xFF7A7B80));
    // An image that does not exist: the default.
    expect(
      parseWallpaperChoice('{"image": "/non/esiste.png"}'),
      isA<GeneratedWallpaper>(),
    );
    final file = File(
      '${Directory.systemTemp.createTempSync().path}/sfondo.png',
    )..writeAsBytesSync([0]);
    expect(
      parseWallpaperChoice('{"image": "${file.path}"}'),
      isA<ImageWallpaper>(),
    );
  });

  testWidgets('the generated wallpaper draws', (tester) async {
    await tester.pumpWidget(const Wallpaper(choice: GeneratedWallpaper()));
    expect(find.byType(CustomPaint), findsWidgets);
  });
}
