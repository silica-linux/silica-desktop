import 'dart:convert';
import 'dart:io';

import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:silica_shell/menubar/menubar.dart';
import 'package:silica_shell/menubar/menubar_config.dart';
import 'package:silica_shell/menubar/status_icons.dart';
import 'package:silica_shell/system/system_status.dart';

import 'controlcenter_test.dart' show FakeSystem;

void main() {
  test('Copy to Menu Bar: setMenubarModule keeps the other keys', () {
    final dir = Directory.systemTemp.createTempSync('mbcfg');
    addTearDown(() => dir.deleteSync(recursive: true));
    final env = {'XDG_CONFIG_HOME': dir.path};
    File(menubarConfigPath(env))
      ..parent.createSync(recursive: true)
      ..writeAsStringSync(jsonEncode({'batteryPercent': true, 'x': 1}));
    expect(menubarShows(readMenubarConfig(env), 'bluetooth'), isFalse);
    expect(setMenubarModule('bluetooth', true, env), isTrue);
    expect(setMenubarModule('sound', true, env), isTrue);
    expect(setMenubarModule('dnd', true, env), isTrue);
    final c = readMenubarConfig(env);
    expect(c.batteryPercent, isTrue);
    expect([
      for (final m in menubarModules) menubarShows(c, m),
    ], everyElement(isTrue));
    expect(jsonDecode(File(menubarConfigPath(env)).readAsStringSync())['x'], 1);
    expect(setMenubarModule('sound', false, env), isTrue);
    expect(readMenubarConfig(env).sound, SoundItem.never);
    expect(setMenubarModule('display', true, env), isFalse);
  });

  test('menubar.json: defaults as on the reference, values read', () {
    expect(MenubarConfig.fromJson(null), const MenubarConfig());
    expect(const MenubarConfig().bluetooth, isFalse);
    expect(const MenubarConfig().sound, SoundItem.active);
    expect(
      MenubarConfig.fromJson({
        'sound': 'always',
        'bluetooth': true,
        'battery': false,
        'batteryPercent': true,
      }),
      const MenubarConfig(
        sound: SoundItem.always,
        bluetooth: true,
        battery: false,
        batteryPercent: true,
      ),
    );
    expect(
      menubarConfigPath({'XDG_CONFIG_HOME': '/c'}),
      '/c/silica/menubar.json',
    );
  });

  testWidgets('Bluetooth and Sound items follow Settings › Menu Bar', (
    tester,
  ) async {
    final system = FakeSystem(
      const SystemSnapshot(
        wifiAvailable: true,
        bluetoothAvailable: true,
        bluetoothPowered: true,
        audioAvailable: true,
        volume: 0.5,
        batteryPresent: true,
        batteryPercent: 64,
      ),
    );
    final config = ValueNotifier(const MenubarConfig());
    await tester.pumpWidget(
      Directionality(
        textDirection: TextDirection.ltr,
        child: MenuBar(
          system: system,
          config: config,
          now: DateTime(2026, 10, 13, 9),
        ),
      ),
    );
    bool shown(StatusKind kind) => tester
        .widgetList<StatusIcon>(find.byType(StatusIcon))
        .any((i) => i.kind == kind);
    // Defaults: no Bluetooth, Sound only while active (nothing plays here).
    expect(shown(StatusKind.bluetooth), isFalse);
    expect(shown(StatusKind.sound), isFalse);
    expect(shown(StatusKind.battery), isTrue);
    expect(find.text('64 %'), findsNothing);

    config.value = const MenubarConfig(
      sound: SoundItem.always,
      bluetooth: true,
      batteryPercent: true,
    );
    await tester.pump();
    expect(shown(StatusKind.bluetooth), isTrue);
    expect(shown(StatusKind.sound), isTrue);
    expect(find.text('64 %'), findsOneWidget);
    final sound = tester
        .widgetList<StatusIcon>(find.byType(StatusIcon))
        .firstWhere((i) => i.kind == StatusKind.sound);
    expect(sound.level, 0.5);

    config.value = const MenubarConfig(battery: false);
    await tester.pump();
    expect(shown(StatusKind.battery), isFalse);
  });
}
