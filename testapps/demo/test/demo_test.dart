import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:silica_demo/main.dart';

void main() {
  Color background(WidgetTester tester) =>
      tester.widget<ColoredBox>(find.byType(ColoredBox).first).color;

  testWidgets('a tap changes color and counter', (tester) async {
    await tester.pumpWidget(const DemoApp());
    expect(find.text('Silica · 0'), findsOneWidget);
    expect(background(tester), idle);

    await tester.tapAt(const Offset(10, 300));
    await tester.pump();
    expect(find.text('Silica · 1'), findsOneWidget);
    expect(background(tester), tapped);
  });

  testWidgets('typing the magic word colors the background, Esc clears it', (
    tester,
  ) async {
    await tester.pumpWidget(const DemoApp());
    await tester.pump();
    await tester.enterText(find.byType(EditableText), magicWord);
    await tester.pump();
    expect(background(tester), typed);

    await tester.sendKeyEvent(LogicalKeyboardKey.escape);
    await tester.pump();
    expect(background(tester), idle);
    expect(find.text(magicWord), findsNothing);
  });

  testWidgets('the probe square follows the devicePixelRatio', (tester) async {
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
