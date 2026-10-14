import 'package:flutter/gestures.dart';
import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:silica_shell/dock/dock.dart';
import 'package:silica_shell/dock/model.dart';
import 'package:silica_ui/silica_ui.dart';
import 'package:silica_shell/main.dart';
import 'package:silica_shell/system/layer_control.dart';
import 'package:silica_shell/system/shell_channel.dart';

void main() {
  testWidgets(
    'the pointer magnifies, shows the label and on exit the Dock goes back to rest',
    (tester) async {
      tester.view.physicalSize = const Size(2560, 200);
      tester.view.devicePixelRatio = 1;
      addTearDown(tester.view.reset);
      await tester.pumpWidget(const ShellComponent(component: 'dock-demo'));

      final rest = layout(placeholderItems, width: 2560, height: 200);
      final mouse = await tester.createGesture(kind: PointerDeviceKind.mouse);
      await mouse.addPointer(location: const Offset(0, 0));
      await mouse.moveTo(Offset(rest.center(6), 155));
      // ~60 ms of entrance.
      await tester.pump(const Duration(milliseconds: 30));
      await tester.pump(const Duration(milliseconds: 40));
      expect(find.text('Photos'), findsOneWidget);

      // Hysteresis: just above the entry threshold but still on the magnified icon, it stays.
      final zoneTop = 200 - bottomGap - backgroundHeight - activeZoneAbove;
      await mouse.moveTo(Offset(rest.center(6), zoneTop - 3));
      await tester.pump(const Duration(milliseconds: 30));
      expect(find.text('Photos'), findsOneWidget);

      // On exit the label stays ~40 ms, then fades in ~250 ms.
      await mouse.moveTo(const Offset(1280, 10));
      await tester.pump(const Duration(milliseconds: 30));
      expect(find.text('Photos'), findsOneWidget);
      await tester.pump(const Duration(milliseconds: 100));
      await tester.pump(const Duration(milliseconds: 300));
      await tester.pump(const Duration(milliseconds: 50));
      expect(find.text('Photos'), findsNothing);
    },
  );

  test('the bounce rises to 17 pt and lands every ~0.7 s', () {
    expect(bounceHeight(0), 0);
    expect(bounceHeight(293), 17);
    expect(bounceHeight(693 + 293), closeTo(17, 0.01));
    expect(bounceHeight(693), 0);
  });

  testWidgets('right click and long press open the menu; the click activates', (
    tester,
  ) async {
    tester.view.physicalSize = const Size(1280, 300);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.reset);
    final log = <String>[];
    const items = [
      Item.app('Terminal', command: 'term'),
      Item.app('Note', command: 'note'),
    ];
    await tester.pumpWidget(
      Directionality(
        textDirection: TextDirection.ltr,
        child: Dock(
          items: items,
          onActivate: (item) => log.add('attiva ${item.label}'),
          menuFor: (item) => [
            MenuAction(
              'Apri ${item.label}',
              onSelected: () => log.add('menu ${item.label}'),
            ),
          ],
        ),
      ),
    );
    final rest = layout(items, width: 1280, height: 300);
    final icon = Offset(rest.center(0), 300 - bottomGap - backgroundHeight / 2);
    // Click: activates.
    await tester.tapAt(icon);
    await tester.pump(const Duration(milliseconds: 50));
    expect(log, ['attiva Terminal']);
    // Right click: the menu, with its tip above the icon.
    await tester.tapAt(icon, buttons: kSecondaryButton);
    await tester.pump();
    expect(find.text('Apri Terminal'), findsOneWidget);
    final body = tester.getRect(find.byType(MenuPanel));
    expect(body.left, closeTo(rest.center(0) - 27, 0.5));
    // Click outside: it closes without choosing.
    await tester.tapAt(const Offset(10, 10));
    await tester.pump(const Duration(milliseconds: 100));
    await tester.pump(const Duration(milliseconds: 200));
    expect(find.text('Apri Terminal'), findsNothing);
    // Long press on the second icon: the menu appears after ~575 ms.
    final second = Offset(rest.center(1), icon.dy);
    final gesture = await tester.startGesture(second);
    await tester.pump(const Duration(milliseconds: 500));
    expect(find.text('Apri Note'), findsNothing);
    await tester.pump(const Duration(milliseconds: 100));
    expect(find.text('Apri Note'), findsOneWidget);
    // Dragging onto the entry and releasing chooses it.
    await gesture.moveTo(tester.getCenter(find.text('Apri Note')));
    await gesture.up();
    await tester.pump(const Duration(milliseconds: 150));
    expect(log, ['attiva Terminal', 'menu Note']);
  });

  testWidgets('the open submenu is part of the input region', (tester) async {
    tester.view.physicalSize = const Size(1280, 300);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.reset);
    final regions = <List<Rect>>[];
    tester.binding.defaultBinaryMessenger.setMockMethodCallHandler(
      LayerControl.channel,
      (call) async {
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
      },
    );
    addTearDown(
      () => tester.binding.defaultBinaryMessenger.setMockMethodCallHandler(
        LayerControl.channel,
        null,
      ),
    );
    final log = <String>[];
    const items = [Item.app('Note', command: 'note')];
    await tester.pumpWidget(
      Directionality(
        textDirection: TextDirection.ltr,
        child: Dock(
          items: items,
          menuFor: (item) => [
            MenuAction(
              'Options',
              submenu: [
                MenuAction(
                  'Remove from Dock',
                  onSelected: () => log.add('via'),
                ),
              ],
            ),
          ],
        ),
      ),
    );
    final rest = layout(items, width: 1280, height: 300);
    final icon = Offset(rest.center(0), 300 - bottomGap - backgroundHeight / 2);
    await tester.tapAt(icon, buttons: kSecondaryButton);
    await tester.pump();
    final mouse = await tester.createGesture(kind: PointerDeviceKind.mouse);
    await mouse.addPointer(location: icon);
    await mouse.moveTo(tester.getCenter(find.text('Options')));
    await tester.pump(const Duration(milliseconds: 350));
    await tester.pump();
    await tester.pump();
    final item = tester.getCenter(find.text('Remove from Dock'));
    expect(regions.last.any((r) => r.contains(item)), isTrue);
  });

  testWidgets(
    'an older Ossido (no transients): with the menu open the Dock takes the whole screen',
    (tester) async {
      tester.view.physicalSize = const Size(1280, 300);
      tester.view.devicePixelRatio = 1;
      addTearDown(tester.view.reset);
      final regions = <List<Object?>>[];
      tester.binding.defaultBinaryMessenger.setMockMethodCallHandler(
        LayerControl.channel,
        (call) async {
          if (call.method == 'outputSize')
            return {'width': 1280, 'height': 800};
          if (call.method == 'setInputRegion') {
            regions.add((call.arguments as Map)['rects'] as List<Object?>);
          }
          return null;
        },
      );
      addTearDown(
        () => tester.binding.defaultBinaryMessenger.setMockMethodCallHandler(
          LayerControl.channel,
          null,
        ),
      );
      const items = [Item.app('Note', command: 'note')];
      await tester.pumpWidget(
        Directionality(
          textDirection: TextDirection.ltr,
          child: Dock(items: items, menuFor: (item) => [MenuAction('Open')]),
        ),
      );
      await tester.pump();
      final rest = layout(items, width: 1280, height: 300);
      final icon = Offset(
        rest.center(0),
        300 - bottomGap - backgroundHeight / 2,
      );
      await tester.tapAt(icon, buttons: kSecondaryButton);
      await tester.pump();
      await tester.pump();
      // A single rectangle, as tall as the screen: the click on the desktop closes the menu.
      expect(regions.last, [
        [0, 0, 1280, 800],
      ]);
    },
  );

  testWidgets(
    'a Dock menu is a transient: the region stays small, a press outside closes it (#149)',
    (tester) async {
      tester.view.physicalSize = const Size(1280, 300);
      tester.view.devicePixelRatio = 1;
      addTearDown(tester.view.reset);
      final layer = <MethodCall>[];
      tester.binding.defaultBinaryMessenger.setMockMethodCallHandler(
        LayerControl.channel,
        (call) async {
          layer.add(call);
          if (call.method == 'outputSize') {
            return {'width': 1280, 'height': 800};
          }
          return call.method == 'setTransient' ? true : null;
        },
      );
      addTearDown(
        () => tester.binding.defaultBinaryMessenger.setMockMethodCallHandler(
          LayerControl.channel,
          null,
        ),
      );
      // The app that was active when the menu opens: its windows get only an activation.
      ShellChannel.instance.toplevels.value = const [
        Toplevel(id: 'w1', title: 'Notes', appId: 'dev.frascella.Note'),
      ];
      ShellChannel.instance.focused.value = 'w1';
      addTearDown(() {
        ShellChannel.instance.toplevels.value = const [];
        ShellChannel.instance.focused.value = '';
      });
      const items = [Item.app('Note', command: 'note')];
      await tester.pumpWidget(
        Directionality(
          textDirection: TextDirection.ltr,
          child: Dock(items: items, menuFor: (item) => [MenuAction('Open')]),
        ),
      );
      await tester.pump();
      expect(layer.any((c) => c.method == 'setTransient'), isFalse);
      final rest = layout(items, width: 1280, height: 300);
      final icon = Offset(
        rest.center(0),
        300 - bottomGap - backgroundHeight / 2,
      );
      await tester.tapAt(icon, buttons: kSecondaryButton);
      await tester.pump();
      await tester.pump();
      Rect rect(Object? r) {
        final v = r as List;
        return Rect.fromLTWH(
          (v[0] as num).toDouble(),
          (v[1] as num).toDouble(),
          (v[2] as num).toDouble(),
          (v[3] as num).toDouble(),
        );
      }

      // The Dock menu's rules (INPUT-01 (c)), and the Dock plus the menu as the
      // rectangles: not the screen.
      final transient = layer.lastWhere((c) => c.method == 'setTransient');
      final args = transient.arguments as Map;
      expect(args['owner'], 'dev.frascella.Note');
      expect(args['ownerRule'], 'activate');
      expect(args['otherRule'], 'pass');
      final rects = [for (final r in args['rects'] as List) rect(r)];
      expect(rects, hasLength(2));
      expect(rects.any((r) => r.contains(icon)), isTrue);
      expect(
        rects.any((r) => r.contains(tester.getCenter(find.text('Open')))),
        isTrue,
      );
      expect(rects.every((r) => r.height < 400), isTrue);
      final region = layer.lastWhere((c) => c.method == 'setInputRegion');
      expect((region.arguments as Map)['rects'], args['rects']);

      // Ossido: a press outside ended it.
      await tester.binding.defaultBinaryMessenger.handlePlatformMessage(
        'silica/shell',
        const JSONMethodCodec().encodeMethodCall(
          const MethodCall('transientDismissed'),
        ),
        (_) {},
      );
      await tester.pump();
      await tester.pump();
      expect(find.text('Open'), findsNothing);
      expect(layer.any((c) => c.method == 'clearTransient'), isTrue);
      final closed = layer.lastWhere((c) => c.method == 'setInputRegion');
      expect(((closed.arguments as Map)['rects'] as List), hasLength(1));
    },
  );

  testWidgets(
    'dragging reorders; up high "Remove" appears and the icon is removed',
    (tester) async {
      tester.view.physicalSize = const Size(1280, 400);
      tester.view.devicePixelRatio = 1;
      addTearDown(tester.view.reset);
      final orders = <String>[];
      const items = [
        Item.app('Uno', command: 'uno', pinned: true),
        Item.app('Due', command: 'due', pinned: true),
        Item.app('Tre', command: 'tre', pinned: true),
        Item.separator(),
        Item.app('Trash', command: 'cestino'),
      ];
      await tester.pumpWidget(
        Directionality(
          textDirection: TextDirection.ltr,
          child: Dock(
            items: items,
            onReorder: (order, dropped) => orders.add(
              '${[for (final i in order) i.label].join(' ')} | ${dropped?.label}',
            ),
          ),
        ),
      );
      final rest = layout(items, width: 1280, height: 400);
      final y = 400 - bottomGap - backgroundHeight / 2;
      // Uno after Due.
      var gesture = await tester.startGesture(Offset(rest.center(0), y));
      await gesture.moveBy(const Offset(10, 0));
      await tester.pump();
      await gesture.moveTo(Offset(rest.center(1) + 20, y));
      await tester.pump();
      await gesture.up();
      await tester.pump(const Duration(milliseconds: 300));
      expect(orders, ['Due Uno Tre | Uno']);
      // Tre up high, more than 130 pt above the grab: "Remove", and on release it disappears.
      gesture = await tester.startGesture(Offset(rest.center(2), y));
      await gesture.moveBy(const Offset(0, -10));
      await tester.pump();
      await gesture.moveTo(Offset(rest.center(2), y - 140));
      await tester.pump();
      expect(find.text('Remove'), findsOneWidget);
      await gesture.up();
      await tester.pump(const Duration(milliseconds: 100));
      expect(orders.last, 'Uno Due | null');
      await tester.pump(const Duration(milliseconds: 300));
      await tester.pump(const Duration(milliseconds: 50));
      expect(find.text('Remove'), findsNothing);
    },
  );

  testWidgets('the fixed file manager is not dragged, nothing goes before it', (
    tester,
  ) async {
    tester.view.physicalSize = const Size(1280, 400);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.reset);
    final orders = <String>[];
    const items = [
      Item.app('Files', command: 'files', pinned: true, fixed: true),
      Item.app('Due', command: 'due', pinned: true),
      Item.app('Tre', command: 'tre', pinned: true),
      Item.separator(),
      Item.app('Trash', command: 'cestino'),
    ];
    await tester.pumpWidget(
      Directionality(
        textDirection: TextDirection.ltr,
        child: Dock(
          items: items,
          onReorder: (order, dropped) => orders.add(
            '${[for (final i in order) i.label].join(' ')} | ${dropped?.label}',
          ),
        ),
      ),
    );
    final rest = layout(items, width: 1280, height: 400);
    final y = 400 - bottomGap - backgroundHeight / 2;
    // Dragging the file manager does nothing, also up high.
    var gesture = await tester.startGesture(Offset(rest.center(0), y));
    await gesture.moveBy(const Offset(10, 0));
    await tester.pump();
    await gesture.moveTo(Offset(rest.center(0), y - 140));
    await tester.pump();
    expect(find.text('Remove'), findsNothing);
    await gesture.up();
    await tester.pump(const Duration(milliseconds: 600));
    expect(orders, isEmpty);
    // Tre dropped at the far left lands after it.
    gesture = await tester.startGesture(Offset(rest.center(2), y));
    await gesture.moveBy(const Offset(-10, 0));
    await tester.pump();
    await gesture.moveTo(Offset(rest.center(0) - 20, y));
    await tester.pump();
    await gesture.up();
    await tester.pump(const Duration(milliseconds: 300));
    expect(orders, ['Files Tre Due | Tre']);
  });

  testWidgets(
    'right click on the separator: its menu; the left click nothing',
    (tester) async {
      tester.view.physicalSize = const Size(1280, 300);
      tester.view.devicePixelRatio = 1;
      addTearDown(tester.view.reset);
      final log = <String>[];
      const items = [
        Item.app('Terminal', command: 'term'),
        Item.separator(),
        Item.app('Trash', command: 'trash'),
      ];
      await tester.pumpWidget(
        Directionality(
          textDirection: TextDirection.ltr,
          child: Dock(
            items: items,
            onActivate: (item) => log.add('attiva ${item.label}'),
            menuFor: (item) => item.kind == ItemKind.separator
                ? const [MenuAction('Show recent apps')]
                : const [],
          ),
        ),
      );
      final rest = layout(items, width: 1280, height: 300);
      final separator = Offset(
        rest.center(1),
        300 - bottomGap - backgroundHeight / 2,
      );
      await tester.tapAt(separator);
      await tester.pump(const Duration(milliseconds: 50));
      expect(log, isEmpty);
      expect(find.byType(MenuPanel), findsNothing);
      await tester.tapAt(separator, buttons: kSecondaryButton);
      await tester.pump();
      expect(find.text('Show recent apps'), findsOneWidget);
    },
  );

  testWidgets(
    'files from another app: the cell below darkens, the drop goes to the item',
    (tester) async {
      tester.view.physicalSize = const Size(1280, 300);
      tester.view.devicePixelRatio = 1;
      addTearDown(tester.view.reset);
      final dropped = <String>[];
      const items = [
        Item.app('Terminal', command: 'term'),
        Item.app('Testo', command: 'testo', openFiles: 'testo %F'),
        Item.separator(),
        Item.app('Trash', command: 'xdg-open trash:///', trash: true),
      ];
      await tester.pumpWidget(
        Directionality(
          textDirection: TextDirection.ltr,
          child: Dock(
            items: items,
            onDropFiles: (item, paths) =>
                dropped.add('${item.label}: ${paths.join(',')}'),
          ),
        ),
      );
      Future<void> send(String method, [Object? args]) async {
        await tester.binding.defaultBinaryMessenger.handlePlatformMessage(
          SilicaDrag.channel.name,
          const JSONMethodCodec().encodeMethodCall(MethodCall(method, args)),
          (_) {},
        );
        await tester.pump(const Duration(milliseconds: 100));
      }

      final rest = layout(items, width: 1280, height: 300);
      final y = 300 - bottomGap - backgroundHeight / 2;
      // On Terminal (takes no files) nothing; on the Trash yes.
      await send('enter', {'x': rest.center(0), 'y': y});
      await send('motion', {'x': rest.center(0), 'y': y});
      await send('drop', {
        'x': rest.center(0),
        'y': y,
        'uris': ['file:///tmp/a.txt'],
      });
      expect(dropped, isEmpty);
      await send('enter', {'x': rest.center(3), 'y': y});
      final current = layout(
        items,
        width: 1280,
        height: 300,
        pointerX: rest.center(3),
        amount: 1,
      );
      await send('motion', {'x': current.center(3), 'y': y});
      await send('drop', {
        'x': current.center(3),
        'y': y,
        'uris': ['file:///tmp/a.txt', 'file:///tmp/b%20c.txt'],
      });
      expect(dropped, ['Trash: /tmp/a.txt,/tmp/b c.txt']);
      await tester.pump(const Duration(seconds: 1));
    },
  );
}
