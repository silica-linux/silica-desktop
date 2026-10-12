import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:silica_demo/main.dart';

void main() {
  testWidgets('un tocco cambia colore e contatore', (tester) async {
    await tester.pumpWidget(const DemoApp());
    expect(find.text('Silica · 0'), findsOneWidget);
    ColoredBox box() => tester.widget(find.byType(ColoredBox).first);
    expect(box().color, idle);

    await tester.tap(find.byType(GestureDetector));
    await tester.pump();
    expect(find.text('Silica · 1'), findsOneWidget);
    expect(box().color, tapped);
  });

  testWidgets('il quadrato sonda segue il devicePixelRatio', (tester) async {
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetDevicePixelRatio);
    await tester.pumpWidget(const DemoApp());
    ColoredBox probe() => tester.widget(find.byType(ColoredBox).at(1));
    expect(probe().color, loDpi);

    tester.view.devicePixelRatio = 2;
    await tester.pump();
    expect(probe().color, hiDpi);
  });
}
