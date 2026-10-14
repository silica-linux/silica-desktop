import 'package:flutter/gestures.dart';
import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:silica_shell/controlcenter/controlcenter.dart';
import 'package:silica_shell/controlcenter/detail.dart' show detailWidth;
import 'package:silica_shell/system/shell_channel.dart';
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
  test(
    'the grid follows the measurements: columns at 0 and 152, rows at a 76 pitch',
    () {
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

      // A desktop without battery or backlight: Sound moves up.
      final desktop = layoutModules(const SystemSnapshot(audioAvailable: true));
      expect(desktop.containsKey('battery'), isFalse);
      expect(desktop.containsKey('display'), isFalse);
      expect(desktop['sound']!.rect.top, 228);
    },
  );

  test('the slider fills up to x0 + 10 + v · (L − 20)', () {
    expect(sliderFill(0), 10);
    expect(sliderFill(1), trackLength - 10);
    expect(sliderValue(sliderFill(0.8886)), closeTo(0.8886, 1e-9));
    expect(sliderValue(-5), 0);
    expect(sliderValue(1000), 1);
  });

  test('the opacity follows the measured frames', () {
    expect(tableAt(openOpacity, 80), closeTo(0.53, 1e-9));
    expect(tableAt(openOpacity, 1000), 1);
    expect(tableAt(closeOpacity, 67), closeTo(0.49, 1e-9));
    expect(tableAt(closeOpacity, 1000), 0);
  });

  testWidgets('opens from the command, the modules act, Esc closes', (
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
    // The stack: 14 pt from the right edge, the first module at y 42.
    final wifi = tester.getRect(
      find
          .ancestor(of: find.text('Wi-Fi'), matching: find.byType(Positioned))
          .first,
    );
    expect(wifi.right, lessThanOrEqualTo(1280 - 14 - 152));
    expect(tester.getTopLeft(find.text('Wi-Fi')).dx, 1280 - 14 - 292 + 56);

    // The icon disc turns off; the text opens the detail.
    final wifiText = tester.getTopLeft(find.text('Wi-Fi'));
    await tester.tapAt(wifiText + const Offset(-56 + 14 + 18, 10));
    await tester.pump();
    expect(system.calls, ['wifi false']);
    expect(find.text('Spento'), findsWidgets);
    await tester.tapAt(wifiText + const Offset(-56 + 14 + 18, 10));
    await tester.pump();
    expect(system.calls.last, 'wifi true');

    // A tap halfway along the Sound track.
    final sound = tester.getTopLeft(find.text('Suono'));
    // The Sound track is 44 pt shorter: the outputs circle is on the right.
    await tester.tapAt(
      Offset(
        sound.dx - 16 + trackLeft + (trackLength - outputsShift) / 2,
        sound.dy - 10 + 42,
      ),
    );
    expect(system.calls.last, 'volume 0.50');
    expect(find.byKey(const ValueKey('audio outputs')), findsOneWidget);

    await tester.sendKeyEvent(LogicalKeyboardKey.escape);
    await tester.pumpAndSettle();
    expect(find.text('Wi-Fi'), findsNothing);
  });

  testWidgets(
    'the text opens the Wi-Fi detail; Esc goes back to the Center, then closes',
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
      // The panel: 308 pt, 10 left of the stack, 6 above; the known networks.
      expect(find.text('Reti conosciute'), findsOneWidget); // Home and Office
      expect(find.text('Ufficio'), findsOneWidget);
      expect(find.text('Bar'), findsNothing);
      final title = tester.getTopLeft(
        find.byWidgetPredicate(
          (w) => w is Text && w.data == 'Wi-Fi' && w.style?.fontSize == 14,
        ),
      );
      expect(title.dx, 1280 - 14 - 292 - 10 + 14);
      // A known network connects; "Altre reti" shows the others.
      await tester.tap(find.text('Ufficio'));
      expect(system.calls.last, 'connect Ufficio');
      await tester.tap(find.text('Altre reti'));
      await tester.pumpAndSettle();
      expect(find.text('Bar'), findsOneWidget);
      // Esc: the Center again, with the modules.
      await tester.sendKeyEvent(LogicalKeyboardKey.escape);
      await tester.pumpAndSettle();
      expect(find.text('Reti conosciute'), findsNothing);
      expect(find.text('Casa'), findsOneWidget);
      await tester.sendKeyEvent(LogicalKeyboardKey.escape);
      await tester.pumpAndSettle();
      expect(find.text('Wi-Fi'), findsNothing);
    },
  );

  testWidgets('the Sound title opens the outputs', (tester) async {
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

  testWidgets(
    'Wi-Fi status menu: below the icon, Esc closes it and announces it',
    (tester) async {
      tester.view.physicalSize = const Size(1280, 800);
      tester.view.devicePixelRatio = 1;
      addTearDown(tester.view.reset);
      final broadcasts = <String>[];
      tester.binding.defaultBinaryMessenger.setMockMethodCallHandler(
        ShellChannel.channel,
        (call) async {
          if (call.method == 'broadcast') {
            final args = call.arguments as Map;
            broadcasts.add('${args['topic']}=${args['value']}');
          }
          return null;
        },
      );
      final system = FakeSystem(laptop);
      await tester.pumpWidget(
        Directionality(
          textDirection: TextDirection.ltr,
          child: ControlCenter(system: system),
        ),
      );
      Future<void> announce(String value) =>
          tester.binding.defaultBinaryMessenger.handlePlatformMessage(
            'silica/shell',
            const JSONMethodCodec().encodeMethodCall(
              MethodCall('broadcast', {'topic': 'statusmenu', 'value': value}),
            ),
            (_) {},
          );
      await announce('wifi 1200');
      await tester.pump();
      await tester.pump();
      final menu = tester.getRect(
        find.byKey(const ValueKey('wifi status menu')),
      );
      expect(menu.top, statusMenuTop);
      expect(menu.right, 1200 + statusMenuRight);
      expect(menu.width, detailWidth);
      expect(find.text('Wi-Fi'), findsOneWidget);
      await tester.sendKeyEvent(LogicalKeyboardKey.escape);
      await tester.pump();
      expect(find.byKey(const ValueKey('wifi status menu')), findsNothing);
      expect(broadcasts, contains('statusmenu='));
    },
  );

  testWidgets(
    'media module: 30 × 30 buttons at a 40 pitch, glyphs and hover (CC-04)',
    (tester) async {
      tester.view.physicalSize = const Size(1280, 800);
      tester.view.devicePixelRatio = 1;
      addTearDown(tester.view.reset);
      await tester.pumpWidget(
        Directionality(
          textDirection: TextDirection.ltr,
          child: ControlCenter(system: FakeSystem(laptop)),
        ),
      );
      await sendCommand(tester, 'controlcenter.toggle');
      await tester.pumpAndSettle();
      final previous = tester.getRect(find.byKey(const ValueKey('precedente')));
      final play = tester.getRect(find.byKey(const ValueKey('riproduci')));
      final next = tester.getRect(find.byKey(const ValueKey('successivo')));
      expect(previous.size, const Size(30, 30));
      expect(play.center.dx - previous.center.dx, 40);
      expect(next.center.dx - play.center.dx, 40);
      // With no players everything is disabled: white ~35 % (`#9d9ea3` over the background).
      final painter =
          tester
                  .widget<CustomPaint>(
                    find
                        .descendant(
                          of: find.byKey(const ValueKey('riproduci')),
                          matching: find.byType(CustomPaint),
                        )
                        .first,
                  )
                  .painter!
              as dynamic;
      expect((painter.color as Color).a, closeTo(mediaGlyphDisabled, 0.01));
      // The pointer on the module brightens it (+7 levels).
      final mouse = await tester.createGesture(kind: PointerDeviceKind.mouse);
      await mouse.addPointer(location: Offset.zero);
      await mouse.moveTo(play.center);
      await tester.pump();
      expect(find.byKey(const ValueKey('media under pointer')), findsOneWidget);
      await mouse.moveTo(Offset.zero);
      await tester.pump();
      expect(find.byKey(const ValueKey('media under pointer')), findsNothing);
      await mouse.removePointer();
    },
  );
}
