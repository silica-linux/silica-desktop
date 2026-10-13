import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:silica_shell/menubar/menubar.dart';
import 'package:silica_shell/system/desktop_entries.dart';
import 'package:silica_shell/menubar/status_icons.dart';
import 'package:silica_shell/system/shell_channel.dart';
import 'package:silica_shell/system/system_status.dart';

import 'controlcenter_test.dart' show FakeSystem;

void main() {
  test("l'orologio come nella barra: giorno, data, doppio spazio, ora", () {
    expect(clockText(DateTime(2026, 10, 12, 22, 34)), 'Lun 12 ott  22:34');
    expect(clockText(DateTime(2026, 1, 4, 9, 5)), 'Dom 4 gen  09:05');
  });

  testWidgets(
    "il nome dell'app con il focus, in grassetto, dalle voci .desktop",
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
      expect(find.text('Lun 12 ott  22:34'), findsOneWidget);
    },
  );

  test('gli archi accesi seguono l\'intensità del Wi-Fi', () {
    expect(wifiArcs(0.9), 3);
    expect(wifiArcs(0.5), 2);
    expect(wifiArcs(0.1), 1);
  });

  testWidgets('batteria e rete dallo stato del sistema', (tester) async {
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

    // Wi-Fi spento: l'icona resta, attenuata; via cavo: l'icona della rete cablata.
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
}
