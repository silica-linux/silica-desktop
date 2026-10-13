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

      await mouse.moveTo(const Offset(1280, 10));
      await tester.pump(const Duration(milliseconds: 30));
      await tester.pump(const Duration(milliseconds: 40));
      expect(find.text('Foto'), findsNothing);
    },
  );
}
