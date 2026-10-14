import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:silica_shell/faro/faro.dart';
import 'package:silica_shell/faro/folders.dart';
import 'package:silica_shell/system/desktop_entries.dart';

const entries = {
  'term': DesktopEntry(id: 'term', name: 'Terminal', exec: 'terminale'),
  'calc': DesktopEntry(id: 'calc', name: 'Calculator', exec: 'calc'),
  'text': DesktopEntry(id: 'text', name: 'Text editor', exec: 'editor'),
};

Future<void> shellMessage(WidgetTester tester, MethodCall call) =>
    tester.binding.defaultBinaryMessenger.handlePlatformMessage(
      'silica/shell',
      const JSONMethodCodec().encodeMethodCall(call),
      (_) {},
    );

/// Records the calls to the layer; `supported` is what Ossido answers to setTransient.
List<MethodCall> mockLayer(WidgetTester tester, {bool supported = true}) {
  final layer = <MethodCall>[];
  tester.binding.defaultBinaryMessenger.setMockMethodCallHandler(
    const MethodChannel('silica/layer', JSONMethodCodec()),
    (call) async {
      layer.add(call);
      return call.method == 'setTransient' ? supported : null;
    },
  );
  addTearDown(
    () => tester.binding.defaultBinaryMessenger.setMockMethodCallHandler(
      const MethodChannel('silica/layer', JSONMethodCodec()),
      null,
    ),
  );
  return layer;
}

Rect rectOf(Object? raw) {
  final r = raw as List;
  return Rect.fromLTWH(
    (r[0] as num).toDouble(),
    (r[1] as num).toDouble(),
    (r[2] as num).toDouble(),
    (r[3] as num).toDouble(),
  );
}

Future<void> pumpFaro(WidgetTester tester) async {
  tester.view.physicalSize = const Size(1440, 900);
  tester.view.devicePixelRatio = 1;
  addTearDown(tester.view.reset);
  await tester.pumpWidget(
    Directionality(
      textDirection: TextDirection.ltr,
      child: Faro(
        entries: entries,
        startOpen: true,
        folders: FolderIndex.empty,
      ),
    ),
  );
  await tester.pumpAndSettle();
}

void main() {
  testWidgets(
    'open, Faro takes the pointer only on its panel, as a panel transient (#149)',
    (tester) async {
      final layer = mockLayer(tester);
      await pumpFaro(tester);
      final transient = layer.lastWhere((c) => c.method == 'setTransient');
      final args = transient.arguments as Map;
      expect(args['press'], 'pass_through');
      expect(args['scroll'], 'pass_through');
      final rect = rectOf((args['rects'] as List).first);
      // Centred, 640 pt wide, nowhere near the whole screen.
      expect(rect.width, closeTo(644, 6));
      expect(rect.center.dx, closeTo(720, 1));
      expect(rect.height, lessThan(200));
      final region = layer.lastWhere((c) => c.method == 'setInputRegion');
      expect((region.arguments as Map)['rects'], args['rects']);
    },
  );

  testWidgets('the region follows the panel when the results grow', (
    tester,
  ) async {
    final layer = mockLayer(tester);
    await pumpFaro(tester);
    final before = rectOf(
      ((layer.lastWhere((c) => c.method == 'setTransient').arguments
                  as Map)['rects']
              as List)
          .first,
    );
    await tester.enterText(find.byType(EditableText), 'te');
    await tester.pumpAndSettle();
    final after = rectOf(
      ((layer.lastWhere((c) => c.method == 'setTransient').arguments
                  as Map)['rects']
              as List)
          .first,
    );
    expect(after.height, greaterThan(before.height + 40));
    expect(after.top, closeTo(before.top, 1));
    final region = layer.lastWhere((c) => c.method == 'setInputRegion');
    expect(rectOf(((region.arguments as Map)['rects'] as List).first), after);
  });

  testWidgets(
    'a press outside closes Faro; the shortcut of the same press does not reopen it',
    (tester) async {
      final layer = mockLayer(tester);
      await pumpFaro(tester);
      expect(find.text('Search with Faro'), findsOneWidget);

      await shellMessage(tester, const MethodCall('transientDismissed'));
      await tester.pumpAndSettle();
      expect(find.text('Search with Faro'), findsNothing);
      expect(layer.any((c) => c.method == 'clearTransient'), isTrue);
      expect(
        (layer.lastWhere((c) => c.method == 'setInputRegion').arguments
            as Map)['rects'],
        isEmpty,
      );

      // The toggle that follows within 400 ms is ignored.
      await shellMessage(tester, const MethodCall('command', 'faro.toggle'));
      await tester.pumpAndSettle();
      expect(find.text('Search with Faro'), findsNothing);
      await shellMessage(
        tester,
        const MethodCall('command', 'launcher.toggle'),
      );
      await tester.pumpAndSettle();
      expect(find.text('Search with Faro'), findsNothing);
    },
  );

  testWidgets('a toggle without a dismissal still closes it', (tester) async {
    mockLayer(tester);
    await pumpFaro(tester);
    await shellMessage(tester, const MethodCall('command', 'faro.toggle'));
    await tester.pumpAndSettle();
    expect(find.text('Search with Faro'), findsNothing);
  });

  testWidgets('an older Ossido: Faro takes the whole screen as before', (
    tester,
  ) async {
    final layer = mockLayer(tester, supported: false);
    await pumpFaro(tester);
    final region = layer.lastWhere((c) => c.method == 'setInputRegion');
    expect((region.arguments as Map)['rects'], isNull);
  });
}
