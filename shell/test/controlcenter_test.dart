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

  List<WifiNetwork> networks = const [
    WifiNetwork(
      ssid: 'Casa',
      strength: 70,
      secure: true,
      known: true,
      active: true,
    ),
    WifiNetwork(ssid: 'Ufficio', strength: 40, secure: true, known: true),
    WifiNetwork(ssid: 'Bar', strength: 90),
  ];

  @override
  Future<List<WifiNetwork>> wifiNetworks() async => networks;

  @override
  Future<void> connectWifi(String ssid) async => calls.add('connect $ssid');

  @override
  Future<List<AudioOutput>> audioOutputs() async => const [
    AudioOutput(id: 74, name: 'Altoparlanti', isDefault: true),
    AudioOutput(id: 62, name: 'Cuffie'),
  ];

  @override
  Future<void> setAudioOutput(int id) async => calls.add('output $id');
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

    // Il disco dell'icona spegne; il testo apre il dettaglio.
    final wifiText = tester.getTopLeft(find.text('Wi-Fi'));
    await tester.tapAt(wifiText + const Offset(-56 + 14 + 18, 10));
    await tester.pump();
    expect(system.calls, ['wifi false']);
    expect(find.text('Spento'), findsWidgets);
    await tester.tapAt(wifiText + const Offset(-56 + 14 + 18, 10));
    await tester.pump();
    expect(system.calls.last, 'wifi true');

    // Un tocco a metà della traccia del Suono.
    final sound = tester.getTopLeft(find.text('Suono'));
    // La traccia del Suono è più corta di 44 pt: il cerchio delle uscite sta a destra.
    await tester.tapAt(
      Offset(
        sound.dx - 16 + trackLeft + (trackLength - outputsShift) / 2,
        sound.dy - 10 + 42,
      ),
    );
    expect(system.calls.last, 'volume 0.50');
    expect(find.byKey(const ValueKey('uscite audio')), findsOneWidget);

    await tester.sendKeyEvent(LogicalKeyboardKey.escape);
    await tester.pumpAndSettle();
    expect(find.text('Wi-Fi'), findsNothing);
  });

  testWidgets(
    'il testo apre il dettaglio del Wi-Fi; Esc torna al Centro, poi chiude',
    (tester) async {
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
      await sendCommand(tester, 'controlcenter.toggle');
      await tester.pumpAndSettle();
      await tester.tapAt(tester.getCenter(find.text('Wi-Fi')));
      await tester.pumpAndSettle();
      // Il pannello: 308 pt, 10 a sinistra della pila, 6 sopra; le reti conosciute.
      expect(find.text('Reti conosciute'), findsOneWidget); // Casa e Ufficio
      expect(find.text('Ufficio'), findsOneWidget);
      expect(find.text('Bar'), findsNothing);
      final title = tester.getTopLeft(
        find.byWidgetPredicate(
          (w) => w is Text && w.data == 'Wi-Fi' && w.style?.fontSize == 14,
        ),
      );
      expect(title.dx, 1280 - 14 - 292 - 10 + 14);
      // Una rete conosciuta si collega; "Altre reti" mostra le altre.
      await tester.tap(find.text('Ufficio'));
      expect(system.calls.last, 'connect Ufficio');
      await tester.tap(find.text('Altre reti'));
      await tester.pumpAndSettle();
      expect(find.text('Bar'), findsOneWidget);
      // Esc: di nuovo il Centro, con i moduli.
      await tester.sendKeyEvent(LogicalKeyboardKey.escape);
      await tester.pumpAndSettle();
      expect(find.text('Reti conosciute'), findsNothing);
      expect(find.text('Casa'), findsOneWidget);
      await tester.sendKeyEvent(LogicalKeyboardKey.escape);
      await tester.pumpAndSettle();
      expect(find.text('Wi-Fi'), findsNothing);
    },
  );

  testWidgets('il titolo del Suono apre le uscite', (tester) async {
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
    await sendCommand(tester, 'controlcenter.toggle');
    await tester.pumpAndSettle();
    await tester.tapAt(tester.getCenter(find.text('Suono')));
    await tester.pumpAndSettle();
    expect(find.text('Uscita'), findsOneWidget);
    await tester.tap(find.text('Cuffie'));
    await tester.pump();
    expect(system.calls.last, 'output 62');
  });
}
