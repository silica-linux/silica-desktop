import 'package:flutter/gestures.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:silica_shell/dock/model.dart';
import 'package:silica_shell/main.dart';

void main() {
  testWidgets(
    'il puntatore ingrandisce, mostra l\'etichetta e all\'uscita il Dock torna a riposo',
    (tester) async {
      tester.view.physicalSize = const Size(2560, 200);
      tester.view.devicePixelRatio = 1;
      addTearDown(tester.view.reset);
      await tester.pumpWidget(const ShellComponent(component: 'dock-demo'));

      final rest = layout(placeholderItems, width: 2560, height: 200);
      final mouse = await tester.createGesture(kind: PointerDeviceKind.mouse);
      await mouse.addPointer(location: const Offset(0, 0));
      await mouse.moveTo(Offset(rest.center(6), 155));
      // ~60 ms di entrata.
      await tester.pump(const Duration(milliseconds: 30));
      await tester.pump(const Duration(milliseconds: 40));
      expect(find.text('Foto'), findsOneWidget);

      // Isteresi: poco sopra la soglia d'ingresso ma ancora sull'icona ingrandita, resta.
      final zoneTop = 200 - bottomGap - backgroundHeight - activeZoneAbove;
      await mouse.moveTo(Offset(rest.center(6), zoneTop - 3));
      await tester.pump(const Duration(milliseconds: 30));
      expect(find.text('Foto'), findsOneWidget);

      // Uscendo l'etichetta resta ~40 ms, poi sfuma in ~250 ms.
      await mouse.moveTo(const Offset(1280, 10));
      await tester.pump(const Duration(milliseconds: 30));
      expect(find.text('Foto'), findsOneWidget);
      await tester.pump(const Duration(milliseconds: 100));
      await tester.pump(const Duration(milliseconds: 300));
      await tester.pump(const Duration(milliseconds: 50));
      expect(find.text('Foto'), findsNothing);
    },
  );

  test('il rimbalzo sale a 17 pt e torna a terra ogni ~0,7 s', () {
    expect(bounceHeight(0), 0);
    expect(bounceHeight(293), 17);
    expect(bounceHeight(693 + 293), closeTo(17, 0.01));
    expect(bounceHeight(693), 0);
  });
}
