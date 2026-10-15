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

  testWidgets('F2 asks the embedder for a second window', (tester) async {
    final calls = <MethodCall>[];
    tester.binding.defaultBinaryMessenger.setMockMethodCallHandler(
      windowChannel,
      (call) async {
        calls.add(call);
        // No view to build in the test: the embedder could not open it.
        return null;
      },
    );
    addTearDown(
      () => tester.binding.defaultBinaryMessenger.setMockMethodCallHandler(
        windowChannel,
        null,
      ),
    );
    await tester.pumpWidget(const DemoApp());
    await tester.sendKeyEvent(LogicalKeyboardKey.f2);
    await tester.pump();
    expect(calls.single.method, 'openWindow');
    expect(calls.single.arguments, {
      'title': secondTitle,
      'width': secondSize.width,
      'height': secondSize.height,
      'minWidth': 150,
      'minHeight': 100,
      'parent': 0,
    });
    // The main window is unchanged.
    expect(background(tester), idle);
  });
}
