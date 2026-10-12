import 'package:flutter/gestures.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:silica_ui/silica_ui.dart';

void main() {
  testWidgets('premuto il vetro si gonfia anche nel compositor, poi torna', (tester) async {
    final regions = <List<Object?>>[];
    tester.binding.defaultBinaryMessenger.setMockMethodCallHandler(
      GlassRegistry.channel,
      (call) async {
        regions.add(call.arguments as List<Object?>);
        return true;
      },
    );
    double width() => ((regions.last.first! as Map)['width'] as num).toDouble();

    await tester.pumpWidget(
      const Directionality(
        textDirection: TextDirection.ltr,
        child: Center(
          child: SizedBox(
            width: 100,
            height: 40,
            child: LiquidGlass(radius: 20, child: SizedBox.expand()),
          ),
        ),
      ),
    );
    await tester.pump();
    expect(width(), 100);

    final gesture = await tester.startGesture(tester.getCenter(find.byType(LiquidGlass)),
        kind: PointerDeviceKind.mouse);
    await tester.pumpAndSettle();
    expect(width(), closeTo(100 * liquidPressScale, 0.5));
    final pressed = (regions.last.first! as Map)['radius'] as num;
    expect(pressed, closeTo(20 * liquidPressScale, 0.1));

    await gesture.up();
    await tester.pumpAndSettle();
    expect(width(), closeTo(100, 0.5));
  });
}
