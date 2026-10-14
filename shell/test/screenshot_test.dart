import 'dart:io';

import 'package:flutter/gestures.dart';
import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:silica_shell/screenshot/screenshot.dart';
import 'package:silica_shell/system/shell_channel.dart';

void main() {
  late List<String> executed;
  late List<Object?> params;

  setUp(() {
    executed = [];
    params = [];
    TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
      ..setMockMethodCallHandler(ShellChannel.channel, (call) async {
        if (call.method == 'execute') {
          final args = call.arguments as Map;
          executed.add('${args['id']}');
          params.add(args['params']);
        }
        return true;
      })
      ..setMockMethodCallHandler(
        const MethodChannel('silica/layer', JSONMethodCodec()),
        (call) async => true,
      );
  });

  Future<void> send(WidgetTester tester, String method, Object? args) =>
      tester.binding.defaultBinaryMessenger.handlePlatformMessage(
        'silica/shell',
        const JSONMethodCodec().encodeMethodCall(MethodCall(method, args)),
        (_) {},
      );

  Future<void> pumpLayer(WidgetTester tester, {String? folder}) async {
    tester.view.physicalSize = const Size(1280, 800);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.reset);
    await tester.pumpWidget(
      Directionality(
        textDirection: TextDirection.ltr,
        child: ScreenshotLayer(
          folder: folder,
          now: () => DateTime(2026, 10, 14, 4, 12, 33),
        ),
      ),
    );
  }

  test('il nome del file, con un numero se esiste già', () {
    final t = DateTime(2026, 10, 14, 4, 12, 33);
    expect(
      screenshotName(t, (_) => false),
      'Schermata del 2026-10-14 alle 04.12.33.png',
    );
    expect(
      screenshotName(t, (n) => !n.contains('(')),
      'Schermata del 2026-10-14 alle 04.12.33 (2).png',
    );
  });

  testWidgets('la miniatura in basso a destra, poi il file sulla Scrivania', (
    tester,
  ) async {
    final dir = Directory.systemTemp.createTempSync('istantanee');
    addTearDown(() => dir.deleteSync(recursive: true));
    final temp = File('${dir.path}/tmp.png')..writeAsBytesSync([0, 1, 2]);
    final desk = Directory('${dir.path}/Scrivania')..createSync();
    await pumpLayer(tester, folder: desk.path);
    await send(tester, 'broadcast', {
      'topic': 'screenshot',
      'value': '${temp.path} 2560 1440',
    });
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 500));
    // L'immagine 256 × 144 con la cornice di 4 pt, a 12 pt dai bordi.
    final thumb = tester.getRect(find.byKey(const ValueKey('miniatura')));
    expect(thumb.size, const Size(264, 152));
    expect(thumb.right, 1280 - 12);
    expect(thumb.bottom, 800 - 12);
    // Resta ~5,1 s, poi esce in ~250 ms e il file va sulla Scrivania.
    await tester.pump(const Duration(seconds: 5));
    expect(temp.existsSync(), isTrue);
    for (var i = 0; i < 10; i++) {
      await tester.pump(const Duration(milliseconds: 50));
    }
    expect(temp.existsSync(), isFalse);
    expect(
      File(
        '${desk.path}/Schermata del 2026-10-14 alle 04.12.33.png',
      ).existsSync(),
      isTrue,
    );
    expect(find.byKey(const ValueKey('miniatura')), findsNothing);
  });

  testWidgets(
    '⇧⌘4: si trascina una regione; Spazio: una finestra; Esc annulla',
    (tester) async {
      await pumpLayer(tester);
      await send(tester, 'command', 'screenshot.selection');
      await tester.pump();
      final mouse = await tester.createGesture(kind: PointerDeviceKind.mouse);
      await mouse.addPointer(location: const Offset(100, 100));
      await mouse.moveTo(const Offset(100, 100));
      await tester.pump();
      expect(find.byKey(const ValueKey('mirino')), findsOneWidget);
      expect(find.text('100\n100'), findsOneWidget);
      await mouse.down(const Offset(100, 100));
      await mouse.moveTo(const Offset(300, 250));
      await tester.pump();
      expect(
        tester.getRect(find.byKey(const ValueKey('selezione'))),
        const Rect.fromLTRB(100, 100, 300, 250),
      );
      await mouse.up();
      await tester.pump();
      await tester.pump(const Duration(milliseconds: 100));
      expect(executed, ['screenshot.region']);
      expect(params.single, {
        'x': 100.0,
        'y': 100.0,
        'width': 200.0,
        'height': 150.0,
      });

      // Spazio: la finestra sotto il puntatore.
      await send(tester, 'command', 'screenshot.selection');
      await tester.pump();
      await tester.sendKeyEvent(LogicalKeyboardKey.space);
      await mouse.moveTo(const Offset(400, 300));
      await tester.pump();
      expect(
        find.byKey(const ValueKey('macchina fotografica')),
        findsOneWidget,
      );
      await mouse.down(const Offset(400, 300));
      await mouse.up();
      await tester.pump();
      await tester.pump(const Duration(milliseconds: 100));
      expect(executed.last, 'screenshot.window_at');
      expect(params.last, {'x': 400.0, 'y': 300.0});

      // Esc: niente.
      await send(tester, 'command', 'screenshot.selection');
      await tester.pump();
      await tester.sendKeyEvent(LogicalKeyboardKey.escape);
      await tester.pump();
      expect(find.byKey(const ValueKey('mirino')), findsNothing);
      expect(executed, hasLength(2));
      await mouse.removePointer();
    },
  );

  testWidgets('⇧⌘5: la barra sopra il Dock; Acquisisci con lo schermo intero', (
    tester,
  ) async {
    await pumpLayer(tester);
    await send(tester, 'command', 'screenshot.toolbar');
    await tester.pump();
    final bar = tester.getRect(
      find
          .ancestor(
            of: find.byKey(const ValueKey('acquisisci')),
            matching: find.byType(Positioned),
          )
          .first,
    );
    expect(bar.size, toolbarSize);
    expect(bar.bottom, 800 - toolbarAboveDock);
    await tester.tap(find.byKey(const ValueKey('schermo intero')));
    await tester.tap(find.byKey(const ValueKey('acquisisci')));
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 100));
    expect(executed, ['screenshot.screen']);
  });
}
