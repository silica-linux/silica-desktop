import 'package:flutter/gestures.dart';
import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:silica_shell/switcher/switcher.dart';
import 'package:silica_shell/system/desktop_entries.dart';
import 'package:silica_shell/system/layer_control.dart';
import 'package:silica_shell/system/shell_channel.dart';

const entries = {
  'term': DesktopEntry(id: 'term', name: 'Terminal', exec: 'terminale'),
  'files': DesktopEntry(id: 'files', name: 'File', exec: 'files'),
};

Toplevel window(String id, String app, [String title = '']) =>
    Toplevel(id: id, title: title, appId: app);

void main() {
  test('apps follow the usage order of the windows', () {
    final model = SwitcherModel(entries: entries)
      ..toplevels([
        window('1', 'term'),
        window('2', 'files'),
        window('3', 'term'),
        window('4', '', 'Senza app'),
      ])
      ..focused('2')
      ..focused('3')
      ..focused('4');
    final apps = model.recentApps();
    expect([for (final a in apps) a.name], ['Senza app', 'Terminal', 'File']);
    // The app's windows from the most recent.
    expect(apps[1].windows, ['3', '1']);
  });

  test('⌘Tab picks the app used before; releasing ⌘ activates it', () {
    final model = SwitcherModel(entries: entries)
      ..toplevels([window('1', 'term'), window('2', 'files')])
      ..focused('1')
      ..focused('2');
    expect(model.visible, isFalse);
    model.next();
    expect(model.visible, isTrue);
    expect(model.apps![model.selected].name, 'Terminal');
    model.next();
    expect(model.selected, 0);
    model.previous();
    expect(model.commit(), '1');
    expect(model.visible, isFalse);
    // ⇧⌘Tab opens on the last one; Esc cancels.
    model.previous();
    expect(model.selected, 1);
    model.cancel();
    expect(model.commit(), isNull);
    // Closed windows leave the order.
    model.toplevels([window('2', 'files')]);
    expect(
      [for (final a in model.recentApps()) a.windows],
      [
        ['2'],
      ],
    );
  });

  testWidgets(
    'four apps: 580 × 176 panel centered, 120 box around the choice',
    (tester) async {
      tester.view.physicalSize = const Size(2560, 1440);
      tester.view.devicePixelRatio = 1;
      addTearDown(tester.view.reset);
      final apps = [
        for (var i = 0; i < 4; i++)
          SwitcherApp(key: '$i', name: 'App $i', windows: ['$i']),
      ];
      await tester.pumpWidget(
        Directionality(
          textDirection: TextDirection.ltr,
          child: SwitcherPanel(apps: apps, selected: 1),
        ),
      );
      expect(panelWidth(4), 580);
      final panel = tester.getRect(find.byType(Positioned).first);
      expect(panel, const Rect.fromLTWH(990, 632, 580, 176));
      final selection = tester.getRect(
        find.byWidgetPredicate(
          (w) =>
              w is DecoratedBox &&
              (w.decoration as BoxDecoration).color == selectionColor,
        ),
      );
      // In the reference x 1153–1272, y 660–779 (measured to ±1 pt).
      expect(selection.left, closeTo(1153, 1.5));
      expect(selection.top, closeTo(660, 1.5));
      expect(selection.size, const Size(120, 120));
      expect(find.text('App 1'), findsOneWidget);
    },
  );

  group('the open switcher', () {
    final apps = {
      'term': parseDesktopEntry('term', '''
[Desktop Entry]
Type=Application
Name=Terminal
Exec=terminale %F
''')!,
      'files': parseDesktopEntry('files', '''
[Desktop Entry]
Type=Application
Name=File
Exec=files %U
''')!,
      'notes': const DesktopEntry(id: 'notes', name: 'Notes', exec: 'notes'),
    };
    late List<MethodCall> calls;
    late List<List<Rect>> regions;

    Future<void> open(WidgetTester tester) async {
      calls = [];
      regions = [];
      tester.view.physicalSize = const Size(2560, 1440);
      tester.view.devicePixelRatio = 1;
      addTearDown(tester.view.reset);
      tester.binding.defaultBinaryMessenger
        ..setMockMethodCallHandler(ShellChannel.channel, (call) async {
          calls.add(call);
          return switch (call.method) {
            'list' => [
              {'id': '1', 'title': '', 'app_id': 'term'},
              {'id': '2', 'title': '', 'app_id': 'files'},
              {'id': '3', 'title': '', 'app_id': 'notes'},
            ],
            'focused' => '3',
            'minimized' => <Object?>[],
            _ => true,
          };
        })
        ..setMockMethodCallHandler(LayerControl.channel, (call) async {
          if (call.method == 'setInputRegion') {
            final rects = (call.arguments as Map)['rects'] as List?;
            regions.add([
              for (final r in rects ?? const [])
                Rect.fromLTWH(
                  (r[0] as num).toDouble(),
                  (r[1] as num).toDouble(),
                  (r[2] as num).toDouble(),
                  (r[3] as num).toDouble(),
                ),
            ]);
          }
          return null;
        });
      addTearDown(() {
        tester.binding.defaultBinaryMessenger
          ..setMockMethodCallHandler(ShellChannel.channel, null)
          ..setMockMethodCallHandler(LayerControl.channel, null);
      });
      await tester.pumpWidget(
        Directionality(
          textDirection: TextDirection.ltr,
          child: AppSwitcher(entries: apps),
        ),
      );
      await tester.pump();
      await tester.pump();
    }

    Future<void> command(WidgetTester tester, String id) async {
      await tester.binding.defaultBinaryMessenger.handlePlatformMessage(
        'silica/shell',
        const JSONMethodCodec().encodeMethodCall(MethodCall('command', id)),
        (_) {},
      );
      await tester.pump();
    }

    Future<void> drag(
      WidgetTester tester,
      String event,
      Offset at, [
      List<String> uris = const [],
    ]) async {
      await tester.binding.defaultBinaryMessenger.handlePlatformMessage(
        'silica/drag',
        const JSONMethodCodec().encodeMethodCall(
          MethodCall(event, {'x': at.dx, 'y': at.dy, 'uris': uris}),
        ),
        (_) {},
      );
      await tester.pump();
    }

    // With 3 apps the panel is 3 * 134 + 44 = 446 wide, centered on 2560 x 1440.
    Offset iconCenter(int i) => Offset(
      (2560 - 446) / 2 + 37 + i * 134 + 52,
      (1440 - 176) / 2 + 35 + 52,
    );

    testWidgets('Shift+Cmd+Tab goes back: it opens on the last app', (
      tester,
    ) async {
      await open(tester);
      await command(tester, 'switcher.previous');
      // Order of use: Notes, File, Terminal. Backwards opens on the last one.
      expect(find.text('Terminal'), findsOneWidget);
      await command(tester, 'switcher.previous');
      expect(find.text('File'), findsOneWidget);
      await command(tester, 'switcher.next');
      expect(find.text('Terminal'), findsOneWidget);
      await command(tester, 'switcher.cancel');
      expect(find.byType(SwitcherPanel), findsNothing);
    });

    testWidgets('Up and Down open App Exposé of the chosen app', (
      tester,
    ) async {
      await open(tester);
      await command(tester, 'switcher.next');
      expect(find.byType(SwitcherPanel), findsOneWidget);
      await command(tester, 'switcher.expose');
      await tester.pump();
      expect(find.byType(SwitcherPanel), findsNothing);
      final names = [for (final c in calls) c.method];
      expect(names.where((m) => m == 'activate'), hasLength(1));
      final execute = calls.lastWhere((c) => c.method == 'execute');
      expect((execute.arguments as Map)['id'], 'panorama.app');
      // Activated first, then Exposé.
      expect(
        names.lastIndexOf('activate'),
        lessThan(names.lastIndexOf('execute')),
      );
    });

    testWidgets('the pointer reaches the panel and moves the choice', (
      tester,
    ) async {
      await open(tester);
      await command(tester, 'switcher.next');
      await tester.pump();
      final panel = regions.last.single;
      expect(panel, const Rect.fromLTWH(1057, 632, 446, 176));
      final mouse = await tester.createGesture(kind: PointerDeviceKind.mouse);
      await mouse.addPointer(location: const Offset(5, 5));
      await mouse.moveTo(iconCenter(2));
      await tester.pump();
      final selection = tester.getRect(
        find.byWidgetPredicate(
          (w) =>
              w is DecoratedBox &&
              (w.decoration as BoxDecoration).color == selectionColor,
        ),
      );
      expect(selection.center.dx, closeTo(iconCenter(2).dx, 1));
      await mouse.removePointer();
      await command(tester, 'switcher.cancel');
      await tester.pump();
      expect(regions.last, isEmpty);
    });

    testWidgets(
      'it works while a file is dragged: dropping on an icon opens it '
      'with that app',
      (tester) async {
        await open(tester);
        // Opened with the keys while the drag is under way.
        await command(tester, 'switcher.next');
        await drag(tester, 'enter', iconCenter(0), ['file:///tmp/a%20b.txt']);
        await drag(tester, 'motion', iconCenter(1));
        final selection = tester.getRect(
          find.byWidgetPredicate(
            (w) =>
                w is DecoratedBox &&
                (w.decoration as BoxDecoration).color == selectionColor,
          ),
        );
        expect(selection.center.dx, closeTo(iconCenter(1).dx, 1));
        await drag(tester, 'drop', iconCenter(1), ['file:///tmp/a%20b.txt']);
        final launch = calls.lastWhere((c) => c.method == 'launch');
        final selected = (launch.arguments as Map)['command'] as String;
        expect(selected, "files 'file:///tmp/a%20b.txt'");
        expect(find.byType(SwitcherPanel), findsNothing);
      },
    );
  });
}
