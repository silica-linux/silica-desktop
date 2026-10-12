import 'package:flutter/gestures.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:silica_dock/main.dart';

void main() {
  test('la curva vale 1 sotto il puntatore e 0 dal raggio in poi', () {
    expect(bump(0), 1);
    expect(bump(1.5), closeTo(0.5, 1e-12));
    expect(bump(3), 0);
    expect(bump(10), 0);
  });

  test('a riposo la fila è larga 664 e centrata', () {
    expect(restRowWidth, 664);
    expect(restCenter(1280, 0), (1280 - 664) / 2 + 24);
    expect(iconSizes(1280, null, 1), everyElement(48));
  });

  test("l'icona sotto il puntatore raddoppia, le lontane no", () {
    final x = restCenter(1280, 5);
    final sizes = iconSizes(1280, x, 1);
    expect(sizes[5], 96);
    expect(sizes[4], closeTo(48 * 1.75, 1e-9));
    expect(sizes[0], 48);
    expect(nearestIcon(1280, x + 20), 5);
    expect(nearestIcon(1280, -100), 0);
  });

  test('i colori seguono la tinta', () {
    final colors = iconColors(0);
    expect(colors.first, HSLColor.fromAHSL(1, 0, 0.7, 0.6).toColor());
  });

  testWidgets('il puntatore ingrandisce e mostra l\'etichetta, poi la molla torna a riposo',
      (tester) async {
    tester.view.physicalSize = const Size(1280, 160);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.reset);
    await tester.pumpWidget(const DockApp());

    final mouse = await tester.createGesture(kind: PointerDeviceKind.mouse);
    await mouse.addPointer(location: const Offset(0, 0));
    await mouse.moveTo(Offset(restCenter(1280, 2), 140));
    await tester.pumpAndSettle();
    expect(find.text('App 3'), findsOneWidget);
    final grown = tester.getSize(find.byType(DecoratedBox).at(2));
    expect(grown.width, closeTo(96, 0.5));

    await mouse.moveTo(const Offset(0, -10));
    await mouse.removePointer();
    await tester.pumpAndSettle();
    expect(find.text('App 3'), findsNothing);
    expect(tester.getSize(find.byType(DecoratedBox).at(2)).width, closeTo(48, 0.5));
  });
}
