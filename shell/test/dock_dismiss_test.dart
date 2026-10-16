// Dock menus and stacks close on a press outside (Ossido's transientDismissed), on a click
// on another tile and on Esc (silica-os/silica-desktop#206).
import 'dart:io';

import 'package:flutter/gestures.dart';
import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:silica_shell/dock/dock.dart';
import 'package:silica_shell/dock/model.dart';
import 'package:silica_shell/dock/stack_settings.dart';
import 'package:silica_shell/system/layer_control.dart';
import 'package:silica_ui/silica_ui.dart';

const _items = [
  Item.app('Note', command: 'note'),
  Item.app('Mail', command: 'mail'),
];

List<MethodCall> _setUp(WidgetTester tester) {
  tester.view.physicalSize = const Size(1280, 800);
  tester.view.devicePixelRatio = 1;
  addTearDown(tester.view.reset);
  final layer = <MethodCall>[];
  tester.binding.defaultBinaryMessenger.setMockMethodCallHandler(
    GlassRegistry.channel,
    (call) async => true,
  );
  tester.binding.defaultBinaryMessenger.setMockMethodCallHandler(
    LayerControl.channel,
    (call) async {
      layer.add(call);
      if (call.method == 'outputSize') return {'width': 1280, 'height': 800};
      return call.method == 'setTransient' ? true : null;
    },
  );
  addTearDown(() {
    tester.binding.defaultBinaryMessenger.setMockMethodCallHandler(
      LayerControl.channel,
      null,
    );
    tester.binding.defaultBinaryMessenger.setMockMethodCallHandler(
      GlassRegistry.channel,
      null,
    );
  });
  return layer;
}

Widget _dock(List<Item> items, {int? viewId, StackSettings? stack}) {
  final dock = Dock(
    items: items,
    onOpenPath: (_) {},
    menuFor: (item) => [MenuAction('Open')],
    stackSettingsFor: stack == null ? null : (_) => stack,
  );
  return Directionality(
    textDirection: TextDirection.ltr,
    child: viewId == null
        ? dock
        : LayerScope(control: LayerControl.forView(viewId), child: dock),
  );
}

Offset _tile(List<Item> items, int i) => Offset(
  layout(items, width: 1280, height: 800).center(i),
  800 - bottomGap - backgroundHeight / 2,
);

Future<void> _settle(WidgetTester tester) async {
  for (var i = 0; i < 8; i++) {
    await tester.pump(const Duration(milliseconds: 60));
  }
}

Future<void> _dismissed(WidgetTester tester, {int? viewId}) async {
  await tester.binding.defaultBinaryMessenger.handlePlatformMessage(
    'silica/shell',
    const JSONMethodCodec().encodeMethodCall(
      MethodCall(
        'transientDismissed',
        viewId == null ? null : {'viewId': viewId},
      ),
    ),
    (_) {},
  );
  await _settle(tester);
}

Future<void> _openMenu(WidgetTester tester) async {
  await tester.tapAt(_tile(_items, 0), buttons: kSecondaryButton);
  await tester.pump();
  await tester.pump();
  expect(find.text('Open'), findsOneWidget);
}

void main() {
  testWidgets('a press outside the Dock layer closes the app menu (its view)', (
    tester,
  ) async {
    final layer = _setUp(tester);
    await tester.pumpWidget(_dock(_items, viewId: 3));
    await tester.pump();
    await _openMenu(tester);
    final set = layer.lastWhere((c) => c.method == 'setTransient');
    expect((set.arguments as Map)['viewId'], 3);
    // Another layer's dismissal is not ours.
    await _dismissed(tester, viewId: 4);
    expect(find.text('Open'), findsOneWidget);
    await _dismissed(tester, viewId: 3);
    expect(find.text('Open'), findsNothing);
    expect(layer.any((c) => c.method == 'clearTransient'), isTrue);
  });

  testWidgets('a click on another tile closes the menu', (tester) async {
    _setUp(tester);
    await tester.pumpWidget(_dock(_items));
    await tester.pump();
    await _openMenu(tester);
    await tester.tapAt(_tile(_items, 1));
    await _settle(tester);
    expect(find.text('Open'), findsNothing);
  });

  testWidgets('Esc closes the menu', (tester) async {
    _setUp(tester);
    await tester.pumpWidget(_dock(_items));
    await tester.pump();
    await _openMenu(tester);
    await tester.sendKeyEvent(LogicalKeyboardKey.escape);
    await _settle(tester);
    expect(find.text('Open'), findsNothing);
  });

  testWidgets('Esc closes the menu and the stack when the focus is elsewhere', (
    tester,
  ) async {
    _setUp(tester);
    final dir = Directory.systemTemp.createTempSync('dismiss');
    addTearDown(() => dir.deleteSync(recursive: true));
    File('${dir.path}/f0.txt').writeAsStringSync('x');
    final items = [
      Item.app('Stack', command: 'x', folder: dir.path),
      const Item.app('Mail', command: 'mail'),
    ];
    await tester.pumpWidget(_dock(items));
    await tester.pump();
    await tester.tapAt(_tile(items, 1), buttons: kSecondaryButton);
    await tester.pump();
    await tester.pump();
    expect(find.text('Open'), findsOneWidget);
    FocusManager.instance.primaryFocus?.unfocus();
    await tester.pump();
    await tester.sendKeyEvent(LogicalKeyboardKey.escape);
    await _settle(tester);
    expect(find.text('Open'), findsNothing);

    await tester.tapAt(_tile(items, 0));
    await _settle(tester);
    expect(find.text('f0.txt'), findsOneWidget);
    FocusManager.instance.primaryFocus?.unfocus();
    await tester.pump();
    await tester.sendKeyEvent(LogicalKeyboardKey.escape);
    await _settle(tester);
    expect(find.text('f0.txt'), findsNothing);
  });

  for (final grid in [false, true]) {
    testWidgets(
      '${grid ? 'grid' : 'fan'} stack: outside press, Esc, other tile',
      (tester) async {
        final layer = _setUp(tester);
        final dir = Directory.systemTemp.createTempSync('dismiss');
        addTearDown(() => dir.deleteSync(recursive: true));
        for (var i = 0; i < 3; i++) {
          File('${dir.path}/f$i.txt').writeAsStringSync('x');
        }
        final items = [
          Item.app('Stack', command: 'x', folder: dir.path),
          const Item.app('Mail', command: 'mail'),
        ];
        Future<void> open() async {
          await tester.tapAt(_tile(items, 0));
          await _settle(tester);
          expect(find.text('f0.txt'), findsOneWidget);
        }

        await tester.pumpWidget(
          _dock(
            items,
            stack: grid
                ? const StackSettings(content: StackContent.grid)
                : null,
          ),
        );
        await tester.pump();
        await open();
        expect(layer.any((c) => c.method == 'setTransient'), isTrue);
        await _dismissed(tester);
        expect(find.text('f0.txt'), findsNothing);
        expect(layer.any((c) => c.method == 'clearTransient'), isTrue);

        await open();
        await tester.sendKeyEvent(LogicalKeyboardKey.escape);
        await _settle(tester);
        expect(find.text('f0.txt'), findsNothing);

        await open();
        await tester.tapAt(_tile(items, 1));
        await _settle(tester);
        expect(find.text('f0.txt'), findsNothing);
      },
    );
  }
}
