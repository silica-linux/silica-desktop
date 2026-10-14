import 'dart:io';

import 'package:flutter/gestures.dart';
import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:silica_shell/screenshot/screenshot.dart';
import 'package:silica_ui/silica_ui.dart' show MenuPanel;
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

  Future<void> pumpLayer(
    WidgetTester tester, {
    String? folder,
    Rect? Function(Offset)? windowAt,
  }) async {
    tester.view.physicalSize = const Size(1280, 800);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.reset);
    await tester.pumpWidget(
      Directionality(
        textDirection: TextDirection.ltr,
        child: ScreenshotLayer(
          folder: folder,
          now: () => DateTime(2026, 10, 14, 4, 12, 33),
          windowAt: windowAt,
        ),
      ),
    );
  }

  test('the file name, with a number if it already exists', () {
    final t = DateTime(2026, 10, 14, 4, 12, 33);
    expect(
      screenshotName(t, (_) => false),
      'Screenshot 2026-10-14 at 04.12.33.png',
    );
    expect(
      screenshotName(t, (n) => !n.contains('(')),
      'Screenshot 2026-10-14 at 04.12.33 (2).png',
    );
  });

  testWidgets('the thumbnail at the bottom right, then the file on the Desktop', (
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
    // The 256 × 144 image with the 4 pt frame, 12 pt from the edges.
    final thumb = tester.getRect(find.byKey(const ValueKey('thumbnail')));
    expect(thumb.size, const Size(264, 152));
    expect(thumb.right, 1280 - 12);
    expect(thumb.bottom, 800 - 12);
    // Corners of radius ~4, no scale on entering (K-05).
    final frame = tester.widget<Container>(
      find
          .descendant(
            of: find.byKey(const ValueKey('thumbnail')),
            matching: find.byType(Container),
          )
          .first,
    );
    expect(
      (frame.decoration! as BoxDecoration).borderRadius,
      BorderRadius.circular(thumbnailRadius),
    );
    expect(find.byType(Transform), findsNothing);
    expect(thumbnailLeave, const Duration(milliseconds: 220));
    // It stays ~5.1 s, then leaves in ~250 ms and the file goes to the Desktop.
    await tester.pump(const Duration(seconds: 5));
    expect(temp.existsSync(), isTrue);
    for (var i = 0; i < 10; i++) {
      await tester.pump(const Duration(milliseconds: 50));
    }
    expect(temp.existsSync(), isFalse);
    expect(
      File('${desk.path}/Screenshot 2026-10-14 at 04.12.33.png').existsSync(),
      isTrue,
    );
    expect(find.byKey(const ValueKey('thumbnail')), findsNothing);
  });

  testWidgets('⇧⌘4: drag a region; Space: a window; Esc cancels', (
    tester,
  ) async {
    await pumpLayer(tester);
    await send(tester, 'command', 'screenshot.selection');
    await tester.pump();
    final mouse = await tester.createGesture(kind: PointerDeviceKind.mouse);
    await mouse.addPointer(location: const Offset(100, 100));
    await mouse.moveTo(const Offset(100, 100));
    await tester.pump();
    expect(find.byKey(const ValueKey('crosshair')), findsOneWidget);
    expect(find.text('100\n100'), findsOneWidget);
    await mouse.down(const Offset(100, 100));
    await mouse.moveTo(const Offset(300, 250));
    await tester.pump();
    expect(
      tester.getRect(find.byKey(const ValueKey('selection'))),
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

    // Space: the window under the pointer.
    await send(tester, 'command', 'screenshot.selection');
    await tester.pump();
    await tester.sendKeyEvent(LogicalKeyboardKey.space);
    await mouse.moveTo(const Offset(400, 300));
    await tester.pump();
    expect(find.byKey(const ValueKey('camera')), findsOneWidget);
    await mouse.down(const Offset(400, 300));
    await mouse.up();
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 100));
    expect(executed.last, 'screenshot.window_at');
    expect(params.last, {'x': 400.0, 'y': 300.0});

    // Esc: nothing.
    await send(tester, 'command', 'screenshot.selection');
    await tester.pump();
    await tester.sendKeyEvent(LogicalKeyboardKey.escape);
    await tester.pump();
    expect(find.byKey(const ValueKey('crosshair')), findsNothing);
    expect(executed, hasLength(2));
    await mouse.removePointer();
  });

  testWidgets('⇧⌘5: the bar above the Dock; Capture with the full screen', (
    tester,
  ) async {
    await pumpLayer(tester);
    await send(tester, 'command', 'screenshot.toolbar');
    await tester.pump();
    final bar = tester.getRect(
      find
          .ancestor(
            of: find.byKey(const ValueKey('capture')),
            matching: find.byType(Positioned),
          )
          .first,
    );
    expect(bar.size, toolbarSize);
    expect(bar.bottom, 800 - toolbarAboveDock);
    await tester.tap(find.byKey(const ValueKey('mode screen')));
    await tester.tap(find.byKey(const ValueKey('capture')));
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 100));
    expect(executed, ['screenshot.screen']);
  });

  testWidgets(
    'Space: light blue veil on the window rectangle, or on everything',
    (tester) async {
      const window = Rect.fromLTWH(200, 150, 500, 350);
      await pumpLayer(
        tester,
        windowAt: (p) => window.contains(p) ? window : null,
      );
      await send(tester, 'command', 'screenshot.selection');
      await tester.pump();
      await tester.sendKeyEvent(LogicalKeyboardKey.space);
      final mouse = await tester.createGesture(kind: PointerDeviceKind.mouse);
      await mouse.addPointer(location: const Offset(400, 300));
      await mouse.moveTo(const Offset(400, 300));
      await tester.pump();
      final veil = find.byKey(const ValueKey('veil'));
      expect(tester.getRect(veil), window);
      expect(tester.widget<ColoredBox>(veil).color, const Color(0x8288BAE7));
      // On the desktop: the whole screen.
      await mouse.moveTo(const Offset(1000, 700));
      await tester.pump();
      expect(tester.getRect(veil), const Rect.fromLTWH(0, 0, 1280, 800));
      await tester.sendKeyEvent(LogicalKeyboardKey.escape);
      await tester.pump();
      await mouse.removePointer();
    },
  );

  testWidgets('Options: the menu above the button; delay and thumbnail', (
    tester,
  ) async {
    final dir = Directory.systemTemp.createTempSync('opzioni');
    addTearDown(() => dir.deleteSync(recursive: true));
    await pumpLayer(tester, folder: dir.path);
    await send(tester, 'command', 'screenshot.toolbar');
    await tester.pump();
    final button = tester.getRect(find.byKey(const ValueKey('options')));
    await tester.tap(find.byKey(const ValueKey('options')));
    await tester.pump();
    final menu = tester.getRect(find.byType(MenuPanel));
    // Above the button, with its left edge on the button.
    expect(menu.bottom, lessThan(button.bottom));
    expect(menu.left, closeTo(button.left, 1));
    expect(find.text('Delay'), findsOneWidget);
    await tester.tap(find.text('5 seconds'));
    await tester.pump(const Duration(milliseconds: 500));
    await tester.pump(const Duration(milliseconds: 500));
    // With the delay the capture comes 5 s later.
    await tester.tap(find.byKey(const ValueKey('capture')));
    await tester.pump();
    await tester.pump(const Duration(seconds: 1));
    expect(executed, isEmpty);
    await tester.pump(const Duration(seconds: 5));
    expect(executed, ['screenshot.screen']);

    // Without a thumbnail the file goes straight into the folder.
    await send(tester, 'command', 'screenshot.toolbar');
    await tester.pump();
    await tester.tap(find.byKey(const ValueKey('options')));
    await tester.pump();
    await tester.tap(find.text('Thumbnail after capture'));
    await tester.pump(const Duration(milliseconds: 500));
    await tester.pump(const Duration(milliseconds: 500));
    final temp = File('${dir.path}/tmp.png')..writeAsBytesSync([0]);
    await send(tester, 'broadcast', {
      'topic': 'screenshot',
      'value': '${temp.path} 2560 1440',
    });
    await tester.pump();
    expect(find.byKey(const ValueKey('thumbnail')), findsNothing);
    expect(temp.existsSync(), isFalse);
    expect(
      File('${dir.path}/Screenshot 2026-10-14 at 04.12.33.png').existsSync(),
      isTrue,
    );
  });
}
