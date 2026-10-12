import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:silica_ui/silica_ui.dart';

void main() {
  testWidgets('Glass manda la sua regione a silica/glass', (tester) async {
    final calls = <MethodCall>[];
    tester.binding.defaultBinaryMessenger.setMockMethodCallHandler(
      GlassRegistry.channel,
      (call) async {
        calls.add(call);
        return true;
      },
    );

    await tester.pumpWidget(
      const Directionality(
        textDirection: TextDirection.ltr,
        child: Stack(
          children: [
            Positioned(
              left: 10,
              top: 20,
              width: 100,
              height: 50,
              child: Glass(radius: 22, child: SizedBox.expand()),
            ),
          ],
        ),
      ),
    );
    await tester.pump();

    expect(calls, isNotEmpty);
    final regions = calls.last.arguments as List<Object?>;
    expect(regions, hasLength(1));
    final region = regions.first! as Map<Object?, Object?>;
    expect(region['x'], 10);
    expect(region['y'], 20);
    expect(region['width'], 100);
    expect(region['height'], 50);
    expect(region['radius'], 22);
    expect(region['material'], 'clear');

    // Senza il widget, le regioni si svuotano.
    await tester.pumpWidget(const SizedBox());
    await tester.pump();
    expect(calls.last.arguments, isEmpty);
  });
}
