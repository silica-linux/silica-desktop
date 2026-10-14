import 'dart:async';
import 'dart:convert';
import 'dart:io';
import 'dart:ui' as ui;

import 'package:flutter/gestures.dart';
import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:silica_shell/l10n/l10n.dart';
import 'package:silica_shell/screenshot/files.dart';
import 'package:silica_shell/screenshot/screenshot.dart';
import 'package:silica_shell/screenshot/selection.dart';
import 'package:silica_shell/screenshot/services.dart';
import 'package:silica_shell/screenshot/toolbar.dart' show ShotToolbar;
import 'package:silica_shell/system/desktop_entries.dart';
import 'package:silica_ui/silica_ui.dart' show MenuPanel;

/// The system, recorded.
class FakeServices implements ScreenshotServices {
  final executed = <String>[];
  final params = <Map<String, Object?>?>[];
  final launched = <String>[];
  final sounds = <String>[];
  final copied = <String>[];
  final dragged = <String>[];
  Offset? hotspot;
  bool dragTaken = true;
  final drags = StreamController<String>.broadcast();
  String? folder;

  @override
  Future<bool> execute(String id, [Map<String, Object?>? p]) async {
    executed.add(id);
    params.add(p);
    return true;
  }

  /// The commands other than the settings and the veil.
  List<String> get captures => [
    for (final id in executed)
      if (id != 'screenshot.settings' && id != 'screenshot.window_veil') id,
  ];

  /// The parameters of the last capture command.
  Map<String, Object?>? get lastCapture {
    for (var i = executed.length - 1; i >= 0; i--) {
      if (executed[i] != 'screenshot.settings' &&
          executed[i] != 'screenshot.window_veil') {
        return params[i];
      }
    }
    return null;
  }

  @override
  void launch(String command) => launched.add(command);

  @override
  void playSound(String name) => sounds.add(name);

  @override
  Future<bool> copyImage(String path) async {
    copied.add(path);
    return true;
  }

  @override
  Future<bool> startDrag(
    String path, {
    ui.Image? icon,
    ui.Offset? hotspot,
  }) async {
    dragged.add(path);
    this.hotspot = hotspot;
    return dragTaken;
  }

  @override
  Stream<String> get dragFinished => drags.stream;

  @override
  Future<String?> chooseFolder({required String title, String? start}) async =>
      folder;
}

void main() {
  late FakeServices services;
  late Directory dir;
  late Directory desk;
  late Directory docs;
  var seq = 1000;

  setUp(() {
    services = FakeServices();
    dir = Directory.systemTemp.createTempSync('istantanee');
    desk = Directory('${dir.path}/Scrivania')..createSync();
    docs = Directory('${dir.path}/Documenti')..createSync();
    TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
        .setMockMethodCallHandler(
          const MethodChannel('silica/layer', JSONMethodCodec()),
          (call) async => true,
        );
  });

  tearDown(() => dir.deleteSync(recursive: true));

  Future<void> send(WidgetTester tester, String method, Object? args) =>
      tester.binding.defaultBinaryMessenger.handlePlatformMessage(
        'silica/shell',
        const JSONMethodCodec().encodeMethodCall(MethodCall(method, args)),
        (_) {},
      );

  /// A capture announced by Ossido: a temporary file of the given size.
  Future<File> announce(
    WidgetTester tester, {
    double width = 2560,
    double height = 1440,
    bool clipboard = false,
  }) async {
    final temp = File('${dir.path}/tmp-${seq++}.png')
      ..writeAsBytesSync([0, 1, 2]);
    await send(tester, 'broadcast', {
      'topic': 'screenshot',
      'value': jsonEncode({
        'path': temp.path,
        'width': width,
        'height': height,
        'clipboard': clipboard,
        'seq': seq,
      }),
    });
    await tester.pump();
    return temp;
  }

  Future<void> pumpLayer(
    WidgetTester tester, {
    Rect? Function(Offset)? windowAt,
    Map<String, DesktopEntry> entries = const {},
    Map<String, String>? env,
  }) async {
    tester.view.physicalSize = const Size(1280, 800);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.reset);
    await tester.pumpWidget(
      Directionality(
        textDirection: TextDirection.ltr,
        child: ScreenshotLayer(
          folder: desk.path,
          documents: docs.path,
          cache: '${dir.path}/cache',
          now: () => DateTime(2026, 10, 14, 4, 12, 33),
          windowAt: windowAt,
          services: services,
          settingsFile: File('${dir.path}/screenshot.json'),
          entries: entries,
          env: env ?? {'HOME': dir.path},
        ),
      ),
    );
  }

  ScreenshotSettings savedSettings() =>
      ScreenshotSettings.load(File('${dir.path}/screenshot.json'));

  const deskName = 'Screenshot 2026-10-14 at 04.12.33.png';

  // --- Files and settings -----------------------------------------------------------------

  test('the file name, localized, with a number if it already exists', () {
    final t = DateTime(2026, 10, 14, 4, 12, 33);
    expect(screenshotName(t, (_) => false), deskName);
    expect(
      screenshotName(t, (n) => !n.contains('(')),
      'Screenshot 2026-10-14 at 04.12.33 (2).png',
    );
    expect(
      screenshotName(
        t,
        (_) => false,
        texts: lookupAppLocalizations(const Locale('it')),
      ),
      'Istantanea schermo 2026-10-14 alle 04.12.33.png',
    );
  });

  test('the settings survive a round trip and hostile files', () {
    final s = ScreenshotSettings(
      destination: const ToFolder('/tmp/x'),
      timer: 10,
      showThumbnail: false,
      rememberSelection: false,
      showPointer: true,
      mode: ToolbarChoice.window,
      selection: const Rect.fromLTWH(1, 2, 3, 4),
      barCenter: const Offset(5, 6),
      recentFolders: ['/tmp/x'],
    );
    final back = ScreenshotSettings.fromJson(jsonDecode(jsonEncode(s)));
    expect(back.destination, const ToFolder('/tmp/x'));
    expect(back.timer, 10);
    expect(back.showThumbnail, isFalse);
    expect(back.rememberSelection, isFalse);
    expect(back.showPointer, isTrue);
    expect(back.mode, ToolbarChoice.window);
    expect(back.selection, const Rect.fromLTWH(1, 2, 3, 4));
    expect(back.barCenter, const Offset(5, 6));
    expect(back.recentFolders, ['/tmp/x']);
    final hostile = ScreenshotSettings.fromJson({
      'destination': 42,
      'timer': 7,
      'thumbnail': 'yes',
      'selection': [1, 2, -3, 4],
      'bar': [1],
      'recent_folders': ['relative', 3, '/ok'],
    });
    expect(hostile.destination, const ToDesktop());
    expect(hostile.timer, 0);
    expect(hostile.showThumbnail, isTrue);
    expect(hostile.selection, isNull);
    expect(hostile.barCenter, isNull);
    expect(hostile.recentFolders, ['/ok']);
    expect(ScreenshotSettings.fromJson('nope').mode, ToolbarChoice.selection);
    // Other folders: newest first, at most three.
    final r = ScreenshotSettings();
    for (final f in ['/a', '/b', '/c', '/d', '/b']) {
      r.chooseFolder(f);
    }
    expect(r.recentFolders, ['/b', '/d', '/c']);
    expect(r.destination, const ToFolder('/b'));
  });

  test('the default image viewer comes from mimeapps.list', () {
    const list = '''
[Added Associations]
image/png=other.desktop;
[Default Applications]
image/jpeg=jpeg.desktop
image/png=missing.desktop;org.example.Viewer.desktop;
''';
    expect(
      defaultInMimeapps(list, 'image/png', (id) => id != 'missing'),
      'org.example.Viewer',
    );
    expect(defaultInMimeapps(list, 'image/gif', (_) => true), isNull);
    final config = Directory('${dir.path}/config')..createSync();
    File('${config.path}/mimeapps.list').writeAsStringSync(list);
    expect(
      defaultAppFor(
        'image/png',
        env: {'HOME': dir.path, 'XDG_CONFIG_HOME': config.path},
        exists: (id) => id == 'org.example.Viewer',
      ),
      'org.example.Viewer',
    );
  });

  test('a live selection: Shift keeps a side, Option grows from the center, '
      'Space moves it', () {
    final live = LiveSelection(const Offset(100, 100));
    expect(
      live.update(const Offset(200, 150), shift: false, option: false),
      const Rect.fromLTRB(100, 100, 200, 150),
    );
    // Shift, then a move mostly to the right: the height stays.
    live.update(const Offset(230, 152), shift: true, option: false);
    expect(
      live.update(const Offset(260, 170), shift: true, option: false),
      const Rect.fromLTRB(100, 100, 260, 150),
    );
    // Option: centered on the start.
    expect(
      live.update(const Offset(150, 130), shift: false, option: true),
      const Rect.fromLTRB(50, 70, 150, 130),
    );
    // Space: the whole selection follows the pointer.
    live.moving = const Offset(150, 130);
    expect(
      live.update(const Offset(160, 140), shift: false, option: false),
      const Rect.fromLTRB(110, 110, 160, 140),
    );
  });

  // --- The thumbnail ------------------------------------------------------------------------

  testWidgets(
    'the thumbnail at the bottom right, the sound, then the file on the Desktop',
    (tester) async {
      await pumpLayer(tester);
      final temp = await announce(tester);
      await tester.pump(const Duration(milliseconds: 500));
      expect(services.sounds, ['screen-capture']);
      // The 256 × 144 image with the 4 pt frame, 12 pt from the edges.
      final thumb = tester.getRect(find.byKey(const ValueKey('thumbnail')));
      expect(thumb.size, const Size(264, 152));
      expect(thumb.right, 1280 - 12);
      expect(thumb.bottom, 800 - 12);
      final frame = tester.widget<Container>(
        find.byKey(const ValueKey('thumbnail frame')),
      );
      expect(
        (frame.decoration! as BoxDecoration).borderRadius,
        BorderRadius.circular(thumbnailRadius),
      );
      expect(find.byType(Transform), findsNothing);
      // It stays ~5.1 s, then leaves in ~220 ms and the file goes to the Desktop.
      await tester.pump(const Duration(seconds: 5));
      expect(temp.existsSync(), isTrue);
      for (var i = 0; i < 10; i++) {
        await tester.pump(const Duration(milliseconds: 50));
      }
      expect(temp.existsSync(), isFalse);
      expect(File('${desk.path}/$deskName').existsSync(), isTrue);
      expect(find.byKey(const ValueKey('thumbnail')), findsNothing);
    },
  );

  testWidgets('a clipboard capture is copied, with no thumbnail and no file', (
    tester,
  ) async {
    await pumpLayer(tester);
    final temp = await announce(tester, clipboard: true);
    await tester.pump();
    expect(services.copied, [temp.path]);
    expect(services.sounds, ['screen-capture']);
    expect(find.byKey(const ValueKey('thumbnail')), findsNothing);
    expect(temp.existsSync(), isFalse);
    expect(desk.listSync(), isEmpty);
  });

  testWidgets('a click on the thumbnail saves the file and opens it', (
    tester,
  ) async {
    await pumpLayer(tester);
    await announce(tester);
    await tester.pump(const Duration(milliseconds: 500));
    await tester.tap(find.byKey(const ValueKey('thumbnail')));
    await tester.pump();
    final saved = '${desk.path}/$deskName';
    expect(File(saved).existsSync(), isTrue);
    expect(services.launched.single, contains("gio open '$saved'"));
    expect(find.byKey(const ValueKey('thumbnail')), findsNothing);
  });

  testWidgets('the thumbnail menu: Delete removes the capture', (tester) async {
    await pumpLayer(
      tester,
      entries: {
        fileManagerId: const DesktopEntry(
          id: fileManagerId,
          name: 'Mustache',
          exec: 'mustache',
        ),
      },
    );
    final temp = await announce(tester);
    await tester.pump(const Duration(milliseconds: 500));
    final thumb = find.byKey(const ValueKey('thumbnail'));
    await tester.tap(thumb, buttons: kSecondaryButton);
    await tester.pump();
    expect(find.byType(MenuPanel), findsOneWidget);
    // Whole on the screen, though the thumbnail touches the right edge.
    expect(
      tester.getRect(find.byType(MenuPanel)).right,
      lessThanOrEqualTo(1280),
    );
    for (final item in [
      'Save to Desktop',
      'Save to Documents',
      'Copy to Clipboard',
      'Show in Mustache',
      'Delete',
      'Close',
    ]) {
      expect(find.text(item), findsOneWidget, reason: item);
    }
    // The menu keeps the thumbnail past its time.
    await tester.pump(const Duration(seconds: 7));
    expect(thumb, findsOneWidget);
    await tester.tap(find.text('Delete'));
    await tester.pump(const Duration(milliseconds: 500));
    expect(thumb, findsNothing);
    expect(temp.existsSync(), isFalse);
    expect(desk.listSync(), isEmpty);
    await tester.pump(const Duration(seconds: 6));
    expect(desk.listSync(), isEmpty);
  });

  testWidgets('the thumbnail menu: Save to Documents, Show in the file manager', (
    tester,
  ) async {
    await pumpLayer(
      tester,
      entries: {
        fileManagerId: const DesktopEntry(
          id: fileManagerId,
          name: 'Mustache',
          exec: 'mustache',
        ),
      },
    );
    await announce(tester);
    await tester.pump(const Duration(milliseconds: 500));
    await tester.tap(
      find.byKey(const ValueKey('thumbnail')),
      buttons: kSecondaryButton,
    );
    await tester.pump();
    await tester.tap(find.text('Save to Documents'));
    await tester.pump(const Duration(milliseconds: 500));
    expect(File('${docs.path}/$deskName').existsSync(), isTrue);
    expect(desk.listSync(), isEmpty);

    // The next one, shown in the file manager: saved first, then its folder opened.
    await announce(tester);
    await tester.pump(const Duration(milliseconds: 500));
    await tester.tap(
      find.byKey(const ValueKey('thumbnail')),
      buttons: kSecondaryButton,
    );
    await tester.pump();
    await tester.tap(find.text('Show in Mustache'));
    await tester.pump(const Duration(milliseconds: 500));
    expect(File('${desk.path}/$deskName').existsSync(), isTrue);
    expect(services.launched.last, "mustache '${desk.path}'");
  });

  testWidgets('dragging the thumbnail writes the file first and drags it out', (
    tester,
  ) async {
    await pumpLayer(tester);
    final temp = await announce(tester);
    await tester.pump(const Duration(milliseconds: 500));
    final thumb = find.byKey(const ValueKey('thumbnail'));
    final at = tester.getTopLeft(thumb) + const Offset(40, 30);
    final mouse = await tester.startGesture(at, kind: PointerDeviceKind.mouse);
    await mouse.moveBy(const Offset(-30, -20));
    await tester.pump();
    final saved = '${desk.path}/$deskName';
    expect(temp.existsSync(), isFalse);
    expect(File(saved).existsSync(), isTrue);
    expect(services.dragged, [saved]);
    // Grabbed where the press was, the thumbnail hidden while it travels.
    expect(services.hotspot!.dx, closeTo(40, 1));
    expect(services.hotspot!.dy, closeTo(30, 1));
    expect(thumb, findsNothing);
    // Dropped nowhere: it comes back, the file already in its place.
    services.drags.add('none');
    await tester.pump();
    await tester.pump();
    expect(thumb, findsOneWidget);
    await mouse.up();
    await tester.pump();

    // Dropped into a folder: gone, the file stays where the target put it.
    final again = await tester.startGesture(
      tester.getCenter(thumb),
      kind: PointerDeviceKind.mouse,
    );
    await again.moveBy(const Offset(-40, 0));
    await tester.pump();
    expect(services.dragged, [saved, saved]);
    services.drags.add('copy');
    await tester.pump();
    await again.up();
    await tester.pump(const Duration(seconds: 7));
    expect(thumb, findsNothing);
    expect(File(saved).existsSync(), isTrue);
    expect(desk.listSync(), hasLength(1));
  });

  testWidgets('a two-finger swipe to the right sends the thumbnail away', (
    tester,
  ) async {
    await pumpLayer(tester);
    await announce(tester);
    await tester.pump(const Duration(milliseconds: 500));
    final thumb = find.byKey(const ValueKey('thumbnail'));
    final pad = TestPointer(1, PointerDeviceKind.trackpad);
    final center = tester.getCenter(thumb);
    await tester.sendEventToBinding(pad.panZoomStart(center));
    await tester.sendEventToBinding(
      pad.panZoomUpdate(center, pan: const Offset(25, 2)),
    );
    await tester.sendEventToBinding(
      pad.panZoomUpdate(center, pan: const Offset(50, 3)),
    );
    await tester.sendEventToBinding(pad.panZoomEnd());
    await tester.pump();
    for (var i = 0; i < 6; i++) {
      await tester.pump(const Duration(milliseconds: 50));
    }
    expect(thumb, findsNothing);
    expect(services.dragged, isEmpty, reason: 'a swipe is not a drag');
    expect(File('${desk.path}/$deskName').existsSync(), isTrue);
  });

  testWidgets('the pointer on the thumbnail keeps it', (tester) async {
    await pumpLayer(tester);
    await announce(tester);
    await tester.pump(const Duration(milliseconds: 500));
    final thumb = find.byKey(const ValueKey('thumbnail'));
    final mouse = await tester.createGesture(kind: PointerDeviceKind.mouse);
    await mouse.addPointer(location: tester.getCenter(thumb));
    await tester.pump();
    await tester.pump(const Duration(seconds: 8));
    expect(thumb, findsOneWidget);
    await mouse.moveTo(const Offset(10, 10));
    await tester.pump(const Duration(seconds: 6));
    await tester.pump(const Duration(milliseconds: 300));
    expect(thumb, findsNothing);
    await mouse.removePointer();
  });

  // --- ⇧⌘4 ----------------------------------------------------------------------------------

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
    // While dragging, the size of the selection.
    expect(find.text('200\n150'), findsOneWidget);
    await mouse.up();
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 100));
    expect(services.captures, ['screenshot.region']);
    expect(services.lastCapture, {
      'x': 100.0,
      'y': 100.0,
      'width': 200.0,
      'height': 150.0,
      'pointer': false,
    });

    // Space: the window under the pointer, with its shadow, the desktop as the screen.
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
    expect(services.captures.last, 'screenshot.window_at');
    expect(services.lastCapture, {
      'x': 400.0,
      'y': 300.0,
      'shadow': true,
      'or_screen': true,
      'pointer': false,
    });

    // Esc: nothing.
    await send(tester, 'command', 'screenshot.selection');
    await tester.pump();
    await tester.sendKeyEvent(LogicalKeyboardKey.escape);
    await tester.pump();
    expect(find.byKey(const ValueKey('crosshair')), findsNothing);
    // Window mode asked Ossido for the veil on the window under the pointer, then off.
    expect(
      services.executed.where((id) => id == 'screenshot.window_veil'),
      hasLength(2),
    );
    expect(services.captures, hasLength(2));
    await mouse.removePointer();
  });

  testWidgets('⌃⇧⌘4 sends the region to the clipboard', (tester) async {
    await pumpLayer(tester);
    await send(tester, 'command', 'screenshot.selection_clipboard');
    await tester.pump();
    final mouse = await tester.startGesture(
      const Offset(10, 10),
      kind: PointerDeviceKind.mouse,
    );
    await mouse.moveTo(const Offset(60, 50));
    await mouse.up();
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 100));
    expect(services.lastCapture?['clipboard'], isTrue);
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

  // --- ⇧⌘5 ----------------------------------------------------------------------------------

  testWidgets('⇧⌘5: the bar above the Dock; Capture with the full screen', (
    tester,
  ) async {
    await pumpLayer(tester);
    await send(tester, 'command', 'screenshot.toolbar');
    await tester.pump();
    final bar = tester.getRect(find.byType(ShotToolbar));
    expect(bar.size, toolbarSize);
    expect(bar.bottom, 800 - toolbarAboveDock);
    expect(bar.center.dx, 640);
    // The controls at their measured places.
    expect(
      tester.getRect(find.byKey(const ValueKey('mode screen'))).left - bar.left,
      41,
    );
    expect(
      tester.getRect(find.byKey(const ValueKey('mode selection'))).left -
          bar.left,
      137,
    );
    expect(find.byKey(const ValueKey('record screen')), findsOneWidget);
    await tester.tap(find.byKey(const ValueKey('mode screen')));
    await tester.pump();
    expect(savedSettings().mode, ToolbarChoice.screen);
    await tester.tap(find.byKey(const ValueKey('capture')));
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 100));
    expect(services.captures, ['screenshot.screen']);
  });

  testWidgets('⇧⌘5 selection: adjusted, remembered, captured', (tester) async {
    await pumpLayer(tester);
    await send(tester, 'command', 'screenshot.toolbar');
    await tester.pump();
    // The default mode is the selection: half the screen, centered.
    expect(find.byKey(const ValueKey('selection editor')), findsOneWidget);
    // Drag its inside: it moves.
    await tester.dragFrom(const Offset(640, 300), const Offset(-100, -50));
    await tester.pump();
    expect(savedSettings().selection, const Rect.fromLTWH(220, 150, 640, 400));
    // A handle: the bottom-right corner grows.
    await tester.dragFrom(const Offset(860, 550), const Offset(20, 10));
    await tester.pump();
    expect(savedSettings().selection, const Rect.fromLTWH(220, 150, 660, 410));
    await tester.tap(find.byKey(const ValueKey('capture')));
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 100));
    expect(services.lastCapture, {
      'x': 220.0,
      'y': 150.0,
      'width': 660.0,
      'height': 410.0,
      'pointer': false,
    });
    // Opened again: the same selection.
    await send(tester, 'command', 'screenshot.toolbar');
    await tester.pump();
    await tester.tap(find.byKey(const ValueKey('capture')));
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 100));
    expect(services.lastCapture?['width'], 660.0);
  });

  testWidgets('⇧⌘5: the bar is moved by its background and stays there', (
    tester,
  ) async {
    await pumpLayer(tester);
    await send(tester, 'command', 'screenshot.toolbar');
    await tester.pump();
    final before = tester.getRect(find.byType(ShotToolbar));
    // Between the second separator and Options there is only background.
    await tester.dragFrom(
      before.topLeft + const Offset(377, 26),
      const Offset(-200, -300),
    );
    await tester.pump();
    final after = tester.getRect(find.byType(ShotToolbar));
    expect(after.topLeft, before.topLeft + const Offset(-200, -300));
    expect(savedSettings().barCenter, after.center);
    await tester.sendKeyEvent(LogicalKeyboardKey.escape);
    await tester.pump();
    await send(tester, 'command', 'screenshot.toolbar');
    await tester.pump();
    expect(tester.getRect(find.byType(ShotToolbar)), after);
  });

  testWidgets('Options: the menu above the button; every item works', (
    tester,
  ) async {
    await pumpLayer(
      tester,
      entries: {
        fileManagerId: const DesktopEntry(
          id: fileManagerId,
          name: 'Mustache',
          exec: 'mustache',
        ),
      },
    );
    await send(tester, 'command', 'screenshot.toolbar');
    await tester.pump();
    await tester.tap(find.byKey(const ValueKey('mode screen')));
    await tester.pump();
    final button = tester.getRect(find.byKey(const ValueKey('options')));
    Future<void> choose(String item) async {
      await tester.tap(find.byKey(const ValueKey('options')));
      await tester.pump();
      await tester.tap(find.text(item));
      await tester.pump(const Duration(milliseconds: 500));
      await tester.pump(const Duration(milliseconds: 500));
    }

    await tester.tap(find.byKey(const ValueKey('options')));
    await tester.pump();
    final menu = tester.getRect(find.byType(MenuPanel));
    // Above the button, with its left edge on the button.
    expect(menu.bottom, lessThan(button.bottom));
    expect(menu.left, closeTo(button.left, 1));
    for (final item in [
      'Save to',
      'Desktop',
      'Documents',
      'Clipboard',
      'Other Folder…',
      'Delay',
      'None',
      '5 seconds',
      '10 seconds',
      'Thumbnail after capture',
      'Keep Last Selection',
      'Include the Pointer',
    ]) {
      expect(find.text(item), findsOneWidget, reason: item);
    }
    // Esc closes the menu, the bar stays.
    await tester.sendKeyEvent(LogicalKeyboardKey.escape);
    await tester.pump(const Duration(milliseconds: 500));
    expect(find.byType(MenuPanel), findsNothing);
    expect(find.byKey(const ValueKey('options')), findsOneWidget);

    // The pointer: Ossido learns it, and the captures ask for it.
    await choose('Include the Pointer');
    expect(savedSettings().showPointer, isTrue);
    expect(services.executed.last, 'screenshot.settings');
    expect(services.params.last, {'pointer': true});

    // Documents: the next capture goes there.
    await choose('Documents');
    expect(savedSettings().destination, const ToDocuments());

    // Other folder: chosen in the file manager, then first among the places.
    services.folder = dir.path;
    await choose('Other Folder…');
    expect(savedSettings().destination, ToFolder(dir.path));
    expect(find.byKey(const ValueKey('options')), findsOneWidget);
    await choose('Documents');

    // The timer: a countdown, then the capture.
    await choose('5 seconds');
    expect(savedSettings().timer, 5);
    await tester.tap(find.byKey(const ValueKey('capture')));
    await tester.pump();
    expect(find.byKey(const ValueKey('countdown')), findsOneWidget);
    expect(find.text('5'), findsOneWidget);
    await tester.pump(const Duration(seconds: 1));
    expect(find.text('4'), findsOneWidget);
    expect(services.captures, isEmpty);
    await tester.pump(const Duration(seconds: 5));
    await tester.pump(const Duration(milliseconds: 100));
    expect(services.captures, ['screenshot.screen']);
    expect(services.lastCapture, {'pointer': true});
    expect(find.byKey(const ValueKey('countdown')), findsNothing);

    // Without a thumbnail the file goes straight into its place.
    await send(tester, 'command', 'screenshot.toolbar');
    await tester.pump();
    await choose('Thumbnail after capture');
    expect(savedSettings().showThumbnail, isFalse);
    await choose('Keep Last Selection');
    expect(savedSettings().rememberSelection, isFalse);
    final temp = await announce(tester);
    expect(find.byKey(const ValueKey('thumbnail')), findsNothing);
    expect(temp.existsSync(), isFalse);
    expect(File('${docs.path}/$deskName').existsSync(), isTrue);
    // A second Esc closes the bar.
    await tester.sendKeyEvent(LogicalKeyboardKey.escape);
    await tester.pump();
    expect(find.byKey(const ValueKey('options')), findsNothing);
  });

  testWidgets('to the clipboard: copied when the thumbnail leaves', (
    tester,
  ) async {
    File(
      '${dir.path}/screenshot.json',
    ).writeAsStringSync(jsonEncode({'destination': 'clipboard'}));
    await pumpLayer(tester);
    final temp = await announce(tester);
    await tester.pump(const Duration(seconds: 6));
    await tester.pump(const Duration(milliseconds: 300));
    expect(services.copied, [temp.path]);
    expect(temp.existsSync(), isFalse);
    expect(desk.listSync(), isEmpty);
  });

  testWidgets('to the image viewer: opened with it from the cache', (
    tester,
  ) async {
    final config = Directory('${dir.path}/config')..createSync();
    File(
      '${config.path}/mimeapps.list',
    ).writeAsStringSync('[Default Applications]\nimage/png=viewer.desktop\n');
    File(
      '${dir.path}/screenshot.json',
    ).writeAsStringSync(jsonEncode({'destination': 'app:viewer'}));
    await pumpLayer(
      tester,
      env: {'HOME': dir.path, 'XDG_CONFIG_HOME': config.path},
      entries: {
        'viewer': const DesktopEntry(
          id: 'viewer',
          name: 'Viewer',
          exec: 'viewer',
          rawExec: 'viewer %U',
        ),
      },
    );
    await send(tester, 'command', 'screenshot.toolbar');
    await tester.pump();
    await tester.tap(find.byKey(const ValueKey('options')));
    await tester.pump();
    expect(find.text('Viewer'), findsOneWidget);
    await tester.sendKeyEvent(LogicalKeyboardKey.escape);
    await tester.pump(const Duration(milliseconds: 500));
    await tester.sendKeyEvent(LogicalKeyboardKey.escape);
    await tester.pump();
    await announce(tester);
    await tester.pump(const Duration(seconds: 6));
    await tester.pump(const Duration(milliseconds: 300));
    final cached = '${dir.path}/cache/$deskName';
    expect(File(cached).existsSync(), isTrue);
    expect(services.launched.single, "viewer '${Uri.file(cached)}'");
  });
}
