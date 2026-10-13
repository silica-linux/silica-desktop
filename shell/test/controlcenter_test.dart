import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:silica_shell/controlcenter/controlcenter.dart';
import 'package:silica_shell/system/system_status.dart';

class FakeSystem extends ValueNotifier<SystemSnapshot> implements SystemSource {
  FakeSystem(super.value);

  final List<String> calls = [];

  @override
  Future<void> setWifi(bool enabled) async {
    calls.add('wifi $enabled');
    value = value.copyWith(wifiEnabled: enabled);
  }

  @override
  Future<void> setBluetooth(bool powered) async => calls.add('bt $powered');

  @override
  Future<void> setBrightness(double v) async =>
      calls.add('brightness ${v.toStringAsFixed(2)}');

  @override
  Future<void> setVolume(double v) async =>
      calls.add('volume ${v.toStringAsFixed(2)}');

  @override
  Future<void> setMuted(bool muted) async => calls.add('muted $muted');

  @override
  Future<void> setDoNotDisturb(bool enabled) async => calls.add('dnd $enabled');
}

const SystemSnapshot laptop = SystemSnapshot(
  wifiAvailable: true,
  wifiEnabled: true,
  wifiSsid: 'Casa',
  bluetoothAvailable: true,
  bluetoothPowered: true,
  batteryPresent: true,
  batteryPercent: 80,
  batteryCharging: true,
  brightnessAvailable: true,
  brightness: 0.5,
  audioAvailable: true,
  volume: 0.46,
);

Future<void> sendCommand(WidgetTester tester, String id) =>
    tester.binding.defaultBinaryMessenger.handlePlatformMessage(
      'silica/shell',
      const JSONMethodCodec().encodeMethodCall(MethodCall('command', id)),
      (_) {},
    );

void main() {
  test('la griglia segue le misure: colonne a 0 e 152, righe a passo 76', () {
    final slots = layoutModules(laptop);
    expect(slots['wifi']!.rect, const Rect.fromLTWH(0, 0, 140, 64));
    expect(slots['bluetooth']!.rect, const Rect.fromLTWH(0, 76, 140, 64));
    expect(slots['media']!.rect, const Rect.fromLTWH(152, 0, 140, 140));
    expect(slots['dnd']!.rect, const Rect.fromLTWH(0, 152, 140, 64));
    expect(slots['battery']!.rect, const Rect.fromLTWH(152, 152, 140, 64));
    expect(slots['display']!.rect, const Rect.fromLTWH(0, 228, 292, 64));
    expect(slots['sound']!.rect, const Rect.fromLTWH(0, 304, 292, 64));
    expect(slots['wifi']!.radius, 32);
    expect(slots['display']!.radius, 28);

    // Un fisso senza batteria né retroilluminazione: Suono sale.
    final desktop = layoutModules(const SystemSnapshot(audioAvailable: true));
    expect(desktop.containsKey('battery'), isFalse);
    expect(desktop.containsKey('display'), isFalse);
    expect(desktop['sound']!.rect.top, 228);
  });

  test('il cursore riempie fino a x0 + 10 + v · (L − 20)', () {
    expect(sliderFill(0), 10);
    expect(sliderFill(1), trackLength - 10);
    expect(sliderValue(sliderFill(0.8886)), closeTo(0.8886, 1e-9));
    expect(sliderValue(-5), 0);
    expect(sliderValue(1000), 1);
  });

  test("l'opacità segue i fotogrammi misurati", () {
    expect(tableAt(openOpacity, 80), closeTo(0.53, 1e-9));
    expect(tableAt(openOpacity, 1000), 1);
    expect(tableAt(closeOpacity, 67), closeTo(0.49, 1e-9));
    expect(tableAt(closeOpacity, 1000), 0);
  });

  testWidgets('si apre dal comando, i moduli agiscono, Esc chiude', (
    tester,
  ) async {
    tester.view.physicalSize = const Size(1280, 800);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.reset);
    final system = FakeSystem(laptop);
    await tester.pumpWidget(
      Directionality(
        textDirection: TextDirection.ltr,
        child: ControlCenter(system: system),
      ),
    );
    expect(find.text('Wi-Fi'), findsNothing);

    await sendCommand(tester, 'controlcenter.toggle');
    await tester.pumpAndSettle();
    expect(find.text('Casa'), findsOneWidget);
    expect(find.text('80% · in carica'), findsOneWidget);
    // La pila: 14 pt dal bordo destro, il primo modulo a y 42.
    final wifi = tester.getRect(
      find
          .ancestor(of: find.text('Wi-Fi'), matching: find.byType(Positioned))
          .first,
    );
    expect(wifi.right, lessThanOrEqualTo(1280 - 14 - 152));
    expect(tester.getTopLeft(find.text('Wi-Fi')).dx, 1280 - 14 - 292 + 56);

    await tester.tap(find.text('Wi-Fi'));
    await tester.pump();
    expect(system.calls, ['wifi false']);
    expect(find.text('Spento'), findsWidgets);

    // Un tocco a metà della traccia del Suono.
    final sound = tester.getTopLeft(find.text('Suono'));
    await tester.tapAt(
      Offset(sound.dx - 16 + trackLeft + trackLength / 2, sound.dy - 10 + 42),
    );
    expect(system.calls.last, 'volume 0.50');

    await tester.sendKeyEvent(LogicalKeyboardKey.escape);
    await tester.pumpAndSettle();
    expect(find.text('Wi-Fi'), findsNothing);
  });
}
