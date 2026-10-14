import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:silica_shell/controlcenter/controlcenter.dart';
import 'package:silica_shell/menubar/menubar.dart';
import 'package:silica_shell/system/layer_control.dart';

import 'controlcenter_test.dart' show FakeSystem, laptop;

/// The measured input model while a menu or panel is open (INPUT-01), case by case.
void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  group('policies', () {
    test('(a, b) menus are modal only for their own app', () {
      expect(TransientPolicy.menu.owner, TransientRule.modal);
      expect(TransientPolicy.menu.other, TransientRule.pass);
    });

    test('(c) the Dock menu only reactivates the app that was active', () {
      expect(TransientPolicy.dockMenu.owner, TransientRule.activate);
      expect(TransientPolicy.dockMenu.other, TransientRule.pass);
    });

    test('(d, e, f, h, i) system panels never block', () {
      expect(TransientPolicy.panel.owner, TransientRule.pass);
      expect(TransientPolicy.panel.other, TransientRule.pass);
    });

    test('(g) a popover lets every click through, in its own window too', () {
      expect(TransientPolicy.popover.owner, TransientRule.pass);
      expect(TransientPolicy.popover.other, TransientRule.pass);
    });
  });

  group('menu bar owners', () {
    test('app menus belong to the frontmost app', () {
      final t = menuBarTransient(open: 'Window', frontmost: 'dev.example.Doc');
      expect(t.policy, TransientPolicy.menu);
      expect(t.owner, 'dev.example.Doc');
      expect(
        menuBarTransient(open: 'app', frontmost: 'dev.example.Doc').owner,
        'dev.example.Doc',
      );
    });

    test('the logo menu belongs to the system', () {
      final t = menuBarTransient(open: 'logo', frontmost: 'dev.example.Doc');
      expect(t.policy, TransientPolicy.menu);
      expect(t.owner, systemOwner);
    });

    test('the desktop menu belongs to the desktop owner', () {
      final t = menuBarTransient(
        desktopMenu: true,
        frontmost: 'dev.example.Doc',
      );
      expect(t.policy, TransientPolicy.menu);
      expect(t.owner, desktopOwner);
      expect(desktopOwner, 'desktop-icons');
    });

    test(
      'the battery menu is a status menu, the arrangement one a popover',
      () {
        expect(
          menuBarTransient(open: 'battery', frontmost: 'dev.example.Doc'),
          (policy: TransientPolicy.panel, owner: systemOwner),
        );
        expect(
          menuBarTransient(windowMenu: true, frontmost: 'dev.example.Doc'),
          (policy: TransientPolicy.popover, owner: 'dev.example.Doc'),
        );
      },
    );
  });

  group('LayerControl.transient', () {
    late List<MethodCall> calls;
    setUp(() {
      calls = [];
      TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
          .setMockMethodCallHandler(LayerControl.channel, (call) async {
            calls.add(call);
            return true;
          });
    });
    tearDown(
      () => TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
          .setMockMethodCallHandler(LayerControl.channel, null),
    );

    test('sends the owner, both rules and the version 6 fallback', () async {
      expect(
        await LayerControl.transient(
          rects: const [Rect.fromLTWH(1, 2, 3, 4)],
          owner: 'dev.example.Doc',
        ),
        isTrue,
      );
      expect(calls.single.arguments, {
        'rects': [
          [1.0, 2.0, 3.0, 4.0],
        ],
        'owner': 'dev.example.Doc',
        'ownerRule': 'modal',
        'otherRule': 'pass',
        'press': 'consume',
        'scroll': 'consume',
      });
    });

    test('panels fall back to letting everything through', () async {
      await LayerControl.transient(policy: TransientPolicy.panel);
      final args = calls.single.arguments as Map;
      expect(args['owner'], systemOwner);
      expect(args['press'], 'pass_through');
      expect(args['scroll'], 'pass_through');
    });

    test(
      'the Dock menu falls back to a consumed press, the wheel passing',
      () async {
        await LayerControl.transient(policy: TransientPolicy.dockMenu);
        final args = calls.single.arguments as Map;
        expect(args['ownerRule'], 'activate');
        expect(args['press'], 'consume');
        expect(args['scroll'], 'pass_through');
      },
    );
  });

  testWidgets(
    '(f) a status menu is a system panel: nothing outside is blocked',
    (tester) async {
      tester.view.physicalSize = const Size(1280, 800);
      tester.view.devicePixelRatio = 1;
      addTearDown(tester.view.reset);
      final layer = <MethodCall>[];
      tester.binding.defaultBinaryMessenger.setMockMethodCallHandler(
        LayerControl.channel,
        (call) async {
          layer.add(call);
          return true;
        },
      );
      addTearDown(
        () => tester.binding.defaultBinaryMessenger.setMockMethodCallHandler(
          LayerControl.channel,
          null,
        ),
      );
      await tester.pumpWidget(
        Directionality(
          textDirection: TextDirection.ltr,
          child: ControlCenter(system: FakeSystem(laptop)),
        ),
      );
      await tester.binding.defaultBinaryMessenger.handlePlatformMessage(
        'silica/shell',
        const JSONMethodCodec().encodeMethodCall(
          const MethodCall('broadcast', {
            'topic': 'statusmenu',
            'value': 'wifi 1200',
          }),
        ),
        (_) {},
      );
      await tester.pump();
      await tester.pump();
      await tester.pump();
      final args =
          layer.lastWhere((c) => c.method == 'setTransient').arguments as Map;
      expect(args['owner'], systemOwner);
      expect(args['ownerRule'], 'pass');
      expect(args['otherRule'], 'pass');
      // The keys stay with the menu.
      final keyboard = layer.lastWhere(
        (c) => c.method == 'setKeyboardInteractivity',
      );
      expect((keyboard.arguments as Map)['mode'], 'exclusive');
      await tester.sendKeyEvent(LogicalKeyboardKey.escape);
      await tester.pump();
      await tester.binding.defaultBinaryMessenger.handlePlatformMessage(
        'silica/shell',
        const JSONMethodCodec().encodeMethodCall(
          const MethodCall('broadcast', {'topic': 'statusmenu', 'value': ''}),
        ),
        (_) {},
      );
      await tester.pump();
    },
  );
}
