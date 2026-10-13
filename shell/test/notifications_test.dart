import 'dart:async';

import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:silica_shell/notifications/model.dart';
import 'package:silica_shell/notifications/notifications.dart';
import 'package:silica_shell/notifications/service.dart';

class FakeSource extends ChangeNotifier implements NotificationsSource {
  final Map<int, ShellNotification> items = {};
  final StreamController<ShellNotification> _posted =
      StreamController.broadcast(sync: true);
  final List<String> calls = [];

  void post(ShellNotification n) {
    items[n.id] = n;
    _posted.add(n);
    notifyListeners();
  }

  @override
  Map<int, ShellNotification> get notifications => items;

  @override
  Stream<ShellNotification> get posted => _posted.stream;

  @override
  Future<void> dismiss(int id) async {
    calls.add('dismiss $id');
    items.remove(id);
    notifyListeners();
  }

  @override
  Future<void> invoke(int id, String action) async =>
      calls.add('invoke $id $action');

  @override
  Future<void> clear() async {
    calls.add('clear');
    items.clear();
    notifyListeners();
  }
}

ShellNotification note(
  int id, {
  String app = 'Posta',
  int minute = 0,
  List<(String, String)> actions = const [],
  int urgency = 1,
  bool transient = false,
}) => ShellNotification(
  id: id,
  appName: app,
  summary: 'Titolo $id',
  body: 'Corpo della notifica $id',
  actions: actions,
  urgency: urgency,
  transient: transient,
  timestamp: DateTime(2026, 10, 13, 9, minute),
);

Future<void> pumpLayer(WidgetTester tester, FakeSource source) async {
  tester.view.physicalSize = const Size(1280, 686);
  tester.view.devicePixelRatio = 1;
  addTearDown(tester.view.reset);
  await tester.pumpWidget(
    Directionality(
      textDirection: TextDirection.ltr,
      child: NotificationsLayer(source: source),
    ),
  );
}

Future<void> sendCommand(WidgetTester tester, String id) =>
    tester.binding.defaultBinaryMessenger.handlePlatformMessage(
      'silica/shell',
      const JSONMethodCodec().encodeMethodCall(MethodCall('command', id)),
      (_) {},
    );

void main() {
  test('la struttura D-Bus diventa una notifica', () {
    final n = ShellNotification.fromDBus([
      7, 'Posta', 'mail', 'Titolo', 'Corpo', //
      ['default', 'Apri', 'reply', 'Rispondi'],
      2, '', 'org.example.Mail', '', true, false, 1791878005135, -1,
    ])!;
    expect(n.id, 7);
    expect(n.critical, isTrue);
    expect(n.transient, isTrue);
    expect(n.hasDefault, isTrue);
    expect(n.buttons, [('reply', 'Rispondi')]);
    expect(n.group, 'org.example.Mail');
    expect(ShellNotification.fromDBus([1, 2]), isNull);
    expect(ShellNotification.fromDBus('x'), isNull);
  });

  test(
    'il Centro raggruppa per app, dal più recente, senza le transitorie',
    () {
      final groups = groupNotifications([
        note(1, minute: 1),
        note(2, app: 'Messaggi', minute: 2),
        note(3, minute: 3),
        note(4, minute: 4, transient: true),
      ]);
      expect([for (final g in groups) g.key], ['Posta', 'Messaggi']);
      expect([for (final n in groups.first.items) n.id], [3, 1]);
    },
  );

  test("l'entrata del banner segue la decelerazione misurata", () {
    expect(bannerEnterOffset(0), bannerEnterDistance);
    // Un fotogramma a 75 Hz riduce la distanza residua a ~0,82.
    final ratio = bannerEnterOffset(13.33 * 5) / bannerEnterOffset(13.33 * 4);
    expect(ratio, closeTo(0.82, 0.01));
    expect(bannerEnterOffset(bannerEnterDuration.inMilliseconds.toDouble()), 0);
  });

  testWidgets('un banner entra in alto a destra e se ne va da solo', (
    tester,
  ) async {
    final source = FakeSource();
    await pumpLayer(tester, source);
    source.post(note(1));
    await tester.pump();
    await tester.pump(bannerEnterDuration);
    final card = find.byType(NotificationCard);
    expect(card, findsOneWidget);
    expect(tester.getTopLeft(card), const Offset(1280 - 16 - 344, 30 + 16));
    expect(tester.getSize(card).width, 344);
    expect(find.text('Titolo 1'), findsOneWidget);

    await tester.pump(bannerDuration);
    await tester.pumpAndSettle();
    expect(find.byType(NotificationCard), findsNothing);
    // Resta nel Centro.
    expect(source.items, contains(1));
  });

  testWidgets('le notifiche critiche restano, un clic attiva la predefinita', (
    tester,
  ) async {
    final source = FakeSource();
    await pumpLayer(tester, source);
    source.post(note(2, urgency: 2, actions: [('default', 'Apri')]));
    await tester.pump();
    await tester.pump(bannerEnterDuration);
    await tester.pump(bannerDuration * 3);
    expect(find.text('Titolo 2'), findsOneWidget);
    await tester.tap(find.text('Titolo 2'));
    await tester.pumpAndSettle();
    expect(source.calls, ['invoke 2 default']);
    expect(find.text('Titolo 2'), findsNothing);
  });

  testWidgets('il Centro mostra gruppi impilati, azioni ed elimina tutto', (
    tester,
  ) async {
    final source = FakeSource()
      ..items[1] = note(1, minute: 1)
      ..items[2] = note(2, minute: 2)
      ..items[3] = note(
        3,
        app: 'Calendario',
        minute: 3,
        actions: [('default', 'Apri'), ('snooze', 'Posticipa')],
      );
    await pumpLayer(tester, source);
    await sendCommand(tester, 'notifications.toggle');
    await tester.pumpAndSettle();

    expect(find.text('Notifiche'), findsOneWidget);
    // Calendario è il più recente; Posta è chiuso con una carta sotto.
    expect(find.text('Titolo 3'), findsOneWidget);
    expect(find.text('Titolo 2'), findsOneWidget);
    expect(find.text('Titolo 1'), findsNothing);
    expect(find.text('09:03'), findsOneWidget);
    expect(
      tester.getTopLeft(find.text('Titolo 3')).dy,
      lessThan(tester.getTopLeft(find.text('Titolo 2')).dy),
    );

    await tester.tap(find.text('Posticipa'));
    expect(source.calls, ['invoke 3 snooze']);

    // Un clic sul gruppo lo apre.
    await tester.tap(find.text('Titolo 2'));
    await tester.pumpAndSettle();
    expect(find.text('Titolo 1'), findsOneWidget);

    await tester.tap(find.byType(CloseCircle).first);
    await tester.pumpAndSettle();
    expect(source.calls.last, 'clear');
    expect(find.text('Nessuna notifica'), findsOneWidget);

    await tester.sendKeyEvent(LogicalKeyboardKey.escape);
    await tester.pumpAndSettle();
    expect(find.text('Notifiche'), findsNothing);
  });
}
