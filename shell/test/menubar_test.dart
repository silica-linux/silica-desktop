import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:silica_shell/menubar/menubar.dart';
import 'package:silica_shell/system/desktop_entries.dart';
import 'package:silica_shell/menubar/status_icons.dart';
import 'package:silica_shell/menubar/window_menu.dart';
import 'package:silica_ui/silica_ui.dart' show MenuPanel;
import 'package:silica_shell/system/shell_channel.dart';
import 'package:silica_shell/system/system_status.dart';

import 'controlcenter_test.dart' show FakeSystem;

void main() {
  test('the clock as in the bar: day, date, double space, time', () {
    expect(clockText(DateTime(2026, 10, 12, 22, 34)), 'Mon 12 Oct  22:34');
    expect(clockText(DateTime(2026, 1, 4, 9, 5)), 'Sun 4 Jan  09:05');
    // In Italian as the bar has always shown it.
    expect(
      clockText(DateTime(2026, 10, 12, 22, 34), 'it'),
      'Lun 12 ott  22:34',
    );
    expect(clockText(DateTime(2026, 1, 4, 9, 5), 'it'), 'Dom 4 gen  09:05');
  });

  testWidgets(
    'the name of the focused app, in bold, from the .desktop entries',
    (tester) async {
      tester.binding.defaultBinaryMessenger.setMockMethodCallHandler(
        ShellChannel.channel,
        (call) async => switch (call.method) {
          'list' => [
            {'id': 'w1', 'title': 'Casa', 'app_id': 'org.gnome.Nautilus'},
          ],
          'focused' => 'w1',
          _ => null,
        },
      );
      const files = DesktopEntry(
        id: 'org.gnome.Nautilus',
        name: 'File',
        exec: 'nautilus',
      );
      await tester.pumpWidget(
        Directionality(
          textDirection: TextDirection.ltr,
          child: MenuBar(
            entries: const {'org.gnome.Nautilus': files},
            now: DateTime(2026, 10, 12, 22, 34),
          ),
        ),
      );
      await tester.pump();
      await tester.pump();
      final name = tester.widget<Text>(find.text('File').first);
      expect(name.style?.fontWeight, FontWeight.w600);
      expect(find.text('Mon 12 Oct  22:34'), findsOneWidget);
    },
  );

  test('the lit arcs follow the Wi-Fi strength', () {
    expect(wifiArcs(0.9), 3);
    expect(wifiArcs(0.5), 2);
    expect(wifiArcs(0.1), 1);
  });

  testWidgets('battery and network from the system state', (tester) async {
    final system = FakeSystem(
      const SystemSnapshot(
        wifiAvailable: true,
        wifiEnabled: true,
        wifiSsid: 'Casa',
        wifiStrength: 79,
        batteryPresent: true,
        batteryPercent: 80,
        batteryCharging: true,
      ),
    );
    await tester.pumpWidget(
      Directionality(
        textDirection: TextDirection.ltr,
        child: MenuBar(system: system, now: DateTime(2026, 10, 13, 9)),
      ),
    );
    StatusIcon icon(StatusKind kind) => tester
        .widgetList<StatusIcon>(find.byType(StatusIcon))
        .firstWhere((i) => i.kind == kind);
    expect(icon(StatusKind.battery).level, 0.8);
    expect(icon(StatusKind.battery).charging, isTrue);
    expect(icon(StatusKind.network).level, 0.79);
    expect(icon(StatusKind.network).dim, isFalse);

    // Wi-Fi off: the icon stays, dimmed; wired: the wired network icon.
    system.value = system.value.copyWith(wifiEnabled: false);
    await tester.pump();
    expect(icon(StatusKind.network).dim, isTrue);
    system.value = const SystemSnapshot(wifiAvailable: true, wired: true);
    await tester.pump();
    expect(
      find.byWidgetPredicate(
        (w) => w is StatusIcon && w.kind == StatusKind.wired,
      ),
      findsOneWidget,
    );
    expect(
      find.byWidgetPredicate(
        (w) => w is StatusIcon && w.kind == StatusKind.battery,
      ),
      findsNothing,
    );
  });

  testWidgets('pressing Window opens the menu; the choice runs the command', (
    tester,
  ) async {
    tester.view.physicalSize = const Size(1280, 400);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.reset);
    final executed = <String>[];
    final layer = <String>[];
    tester.binding.defaultBinaryMessenger
      ..setMockMethodCallHandler(ShellChannel.channel, (call) async {
        if (call.method == 'execute') {
          executed.add('${(call.arguments as Map)['id']}');
        }
        return null;
      })
      ..setMockMethodCallHandler(
        const MethodChannel('silica/layer', JSONMethodCodec()),
        (call) async {
          layer.add(call.method);
          return true;
        },
      );
    await tester.pumpWidget(
      Directionality(
        textDirection: TextDirection.ltr,
        child: MenuBar(now: DateTime(2026, 10, 12, 22, 34)),
      ),
    );
    await tester.pump();
    final finestra = tester.getRect(find.text('Window'));
    await tester.tapAt(finestra.center);
    await tester.pump();
    expect(find.text('Minimize'), findsOneWidget);
    // The surface grows and the keyboard goes to the menu.
    expect(
      layer,
      containsAll(['setSize', 'setInputRegion', 'setKeyboardInteractivity']),
    );
    // The menu body 5 pt left of the box (which has ~10.75 pt before the text).
    final riduci = tester.getTopLeft(find.text('Minimize'));
    expect(riduci.dx, closeTo(finestra.left - itemPadding - 5 + 17, 1));
    await tester.tapAt(tester.getCenter(find.text('Minimize')));
    await tester.pump(const Duration(milliseconds: 150));
    expect(executed, ['window.minimize']);
    await tester.pump(const Duration(milliseconds: 100));
    await tester.pump(const Duration(milliseconds: 200));
    expect(find.text('Minimize'), findsNothing);
  });

  testWidgets(
    'long press on the green button: the popover below the button runs the arrangement',
    (tester) async {
      tester.view.physicalSize = const Size(1280, 800);
      tester.view.devicePixelRatio = 1;
      addTearDown(tester.view.reset);
      final executed = <String>[];
      final layer = <String>[];
      tester.binding.defaultBinaryMessenger
        ..setMockMethodCallHandler(ShellChannel.channel, (call) async {
          if (call.method == 'execute') {
            executed.add('${(call.arguments as Map)['id']}');
          }
          return null;
        })
        ..setMockMethodCallHandler(
          const MethodChannel('silica/layer', JSONMethodCodec()),
          (call) async {
            layer.add(call.method);
            return call.method == 'outputSize'
                ? {'width': 1280, 'height': 800}
                : true;
          },
        );
      await tester.pumpWidget(
        Directionality(
          textDirection: TextDirection.ltr,
          child: MenuBar(now: DateTime(2026, 10, 12, 22, 34)),
        ),
      );
      await tester.pump();
      await tester.binding.defaultBinaryMessenger.handlePlatformMessage(
        'silica/shell',
        const JSONMethodCodec().encodeMethodCall(
          MethodCall('windowMenu', {'id': 'w1', 'x': 300, 'y': 200}),
        ),
        (_) {},
      );
      await tester.pump();
      await tester.pump();
      expect(find.text('Half of the screen'), findsOneWidget);
      // The body below the tip: 8 pt below the apex, 27 to the left.
      final panel = tester.getRect(find.byType(MenuPanel));
      expect(panel.top, 208);
      expect(panel.left, 273);
      expect(panel.width, windowMenuWidth);
      await tester.tap(find.byKey(const ValueKey('window.tile_left')));
      // Blink and fade of the closing (~380 ms).
      for (var i = 0; i < 12; i++) {
        await tester.pump(const Duration(milliseconds: 50));
      }
      expect(executed, ['window.tile_left']);
      expect(find.text('Half of the screen'), findsNothing);
    },
  );

  testWidgets(
    'capsules on the controls icon and on the clock with the Centers open',
    (tester) async {
      tester.view.physicalSize = const Size(1280, 400);
      tester.view.devicePixelRatio = 1;
      addTearDown(tester.view.reset);
      tester.binding.defaultBinaryMessenger.setMockMethodCallHandler(
        ShellChannel.channel,
        (call) async => null,
      );
      await tester.pumpWidget(
        Directionality(
          textDirection: TextDirection.ltr,
          child: MenuBar(now: DateTime(2026, 10, 12, 22, 34)),
        ),
      );
      await tester.pump();
      Future<void> announce(String topic, String value) =>
          tester.binding.defaultBinaryMessenger.handlePlatformMessage(
            'silica/shell',
            const JSONMethodCodec().encodeMethodCall(
              MethodCall('broadcast', {'topic': topic, 'value': value}),
            ),
            (_) {},
          );
      const controls = ValueKey('control center capsule');
      const clock = ValueKey('clock capsule');
      expect(find.byKey(controls), findsNothing);
      await announce('controlcenter.open', '1');
      await tester.pump();
      await tester.pump();
      final capsule = tester.getRect(find.byKey(controls));
      expect(capsule.size, controlsCapsule);
      await announce('notifications.open', '1');
      await tester.pump();
      expect(find.byKey(clock), findsOneWidget);
      await announce('controlcenter.open', '0');
      await announce('notifications.open', '0');
      await tester.pump();
      expect(find.byKey(controls), findsNothing);
      expect(find.byKey(clock), findsNothing);
    },
  );
}
