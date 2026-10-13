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

  testWidgets(
    'un antenato che cambia scala sposta il vetro anche senza ridisegnarlo',
    (tester) async {
      final sent = <Map<Object?, Object?>>[];
      tester.binding.defaultBinaryMessenger.setMockMethodCallHandler(
        GlassRegistry.channel,
        (call) async {
          final regions = call.arguments as List<Object?>;
          if (regions.isNotEmpty) {
            sent.add(regions.first! as Map<Object?, Object?>);
          }
          return true;
        },
      );
      Widget at(double scale) => Directionality(
        textDirection: TextDirection.ltr,
        child: Center(
          child: Transform.scale(
            scale: scale,
            // Il vetro non si ridisegna quando cambia solo la trasformazione.
            child: const RepaintBoundary(
              child: SizedBox(
                width: 100,
                height: 50,
                child: Glass(radius: 25, child: SizedBox.expand()),
              ),
            ),
          ),
        ),
      );
      await tester.pumpWidget(at(1.5));
      await tester.pump();
      expect(sent.last['width'], 150);
      await tester.pumpWidget(at(1));
      await tester.pump();
      expect(sent.last['width'], 100);
      expect(sent.last['radius'], 25);
    },
  );
}
