import 'dart:async';

import 'package:flutter_test/flutter_test.dart';
import 'package:silica_shell/dock/launcher_entry.dart';
import 'package:silica_shell/dock/model.dart';
import 'package:silica_shell/system/dbus.dart';

void main() {
  test('LauncherEntry updates: badge, progress, attention', () {
    final now = DateTime(2026, 10, 14, 12);
    var s = applyLauncherUpdate(const LauncherState(), {
      'count': 3,
      'count-visible': true,
    }, now: now);
    expect(s.badge, '3');
    s = applyLauncherUpdate(s, {'count': 1234}, now: now);
    expect(s.badge, '1234');
    s = applyLauncherUpdate(s, {'count-visible': false}, now: now);
    expect(s.badge, isNull);
    s = applyLauncherUpdate(s, {
      'progress': 0.5,
      'progress-visible': true,
    }, now: now);
    expect(s.progress, 0.5);
    s = applyLauncherUpdate(s, {'urgent': true}, now: now);
    expect(s.urgentSince, now);
    // Already urgent: the hops keep their rhythm.
    s = applyLauncherUpdate(s, {
      'urgent': true,
    }, now: now.add(const Duration(seconds: 1)));
    expect(s.urgentSince, now);
    s = applyLauncherUpdate(s, {'urgent': false, 'progress-visible': false});
    expect(s.isEmpty, isTrue);
    expect(launcherAppId('application://firefox.desktop'), 'firefox');
    expect(launcherAppId('file:///x'), isNull);
  });

  test(
    'signals from the bus reach the state; activation stops attention',
    () async {
      final bus = StreamController<DBusSignal>();
      final entries = LauncherEntries(signals: bus.stream)..start();
      bus.add(
        const DBusSignal(
          sender: ':1.5',
          path: '/',
          interface: 'com.canonical.Unity.LauncherEntry',
          member: 'Update',
          args: [
            'application://dev.frascella.Mail.desktop',
            {'count': 7, 'count-visible': true, 'urgent': true},
          ],
        ),
      );
      await Future<void>.delayed(Duration.zero);
      expect(entries.value.value['dev.frascella.Mail']?.badge, '7');
      expect(entries.value.value['dev.frascella.Mail']?.urgentSince, isNotNull);
      entries.activated('dev.frascella.Mail');
      expect(entries.value.value['dev.frascella.Mail']?.urgentSince, isNull);
      expect(entries.value.value['dev.frascella.Mail']?.badge, '7');
      await bus.close();
      entries.dispose();
    },
  );

  test('attention: three decaying hops every 2 s; badge widths', () {
    expect(attentionHeight(233), closeTo(56 * amalfiScale, 0.01));
    expect(attentionHeight(466 + 160), closeTo(23 * amalfiScale, 0.01));
    expect(attentionHeight(466 + 320 + 107), closeTo(8 * amalfiScale, 0.01));
    expect(attentionHeight(1500), 0);
    expect(attentionHeight(2000 + 233), closeTo(56 * amalfiScale, 0.01));
    expect(badgeWidth('1'), 23);
    expect(badgeWidth('12'), 23);
    expect(badgeWidth('123'), 26);
    expect(badgeWidth('1234'), 33);
    expect(badgeWidth('Nuovo'), 39);
  });
}
