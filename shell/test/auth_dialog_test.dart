import 'dart:async';

import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:silica_shell/auth/auth_dialog.dart';
import 'package:silica_shell/system/desktop_entries.dart';

class FakeAuth implements AuthSource {
  final requests = StreamController<AuthRequest>.broadcast();
  final closes = StreamController<int>.broadcast();
  final answers = <String>[];
  final cancelled = <int>[];
  List<AuthRequest> waiting = const [];

  @override
  Future<List<AuthRequest>> pending() async => waiting;
  @override
  Stream<AuthRequest> get requested => requests.stream;
  @override
  Stream<int> get closed => closes.stream;
  @override
  Future<bool> respond(int id, String user, String password) async {
    answers.add('$id $user $password');
    return password == 'secret';
  }

  @override
  Future<void> cancel(int id) async => cancelled.add(id);
}

const request = AuthRequest(
  id: 4,
  action: 'org.freedesktop.timedate1.set-timezone',
  message: 'Authentication is required to set the system timezone.',
  app: 'settings',
  users: {'root': 'root', 'anna': 'Anna Rossi'},
  defaultUser: 'anna',
);

Future<void> pump(WidgetTester tester, FakeAuth auth) async {
  tester.view.physicalSize = const Size(1280, 800);
  tester.view.devicePixelRatio = 1;
  addTearDown(tester.view.reset);
  tester.binding.defaultBinaryMessenger.setMockMethodCallHandler(
    const MethodChannel('silica/layer', JSONMethodCodec()),
    (call) async => true,
  );
  await tester.pumpWidget(
    Directionality(
      textDirection: TextDirection.ltr,
      child: AuthLayer(
        source: auth,
        entries: const {
          'dev.frascella.Settings': DesktopEntry(
            id: 'dev.frascella.Settings',
            name: 'Settings',
            exec: '/usr/bin/settings %U',
          ),
        },
      ),
    ),
  );
  await tester.pump();
}

void main() {
  test('requests are read from the D-Bus tuple', () {
    final parsed = AuthRequest.fromDBus([
      7,
      'a.b',
      'Message',
      'icon',
      'settings',
      [
        ['anna', 'Anna Rossi'],
        ['root', 'root'],
      ],
      'anna',
    ])!;
    expect(parsed.id, 7);
    expect(parsed.users, {'anna': 'Anna Rossi', 'root': 'root'});
    expect(parsed.defaultUser, 'anna');
    expect(AuthRequest.fromDBus(['x']), isNull);
    expect(AuthRequest.fromDBus(null), isNull);
  });

  testWidgets('a wrong password shakes, the right one closes', (tester) async {
    final auth = FakeAuth();
    await pump(tester, auth);
    expect(find.byKey(const ValueKey('auth 4')), findsNothing);
    auth.requests.add(request);
    await tester.pump();
    await tester.pump();
    expect(find.byKey(const ValueKey('auth 4')), findsOneWidget);
    // The app's name from its desktop entry, the user running the session filled in.
    expect(
      find.text('Settings needs an administrator’s permission.'),
      findsOneWidget,
    );
    expect(find.text('anna'), findsOneWidget);

    await tester.enterText(find.byKey(const ValueKey('auth password')), 'nope');
    await tester.tap(find.byKey(const ValueKey('auth allow')));
    await tester.pump();
    expect(auth.answers, ['4 anna nope']);
    // Still open, the field cleared, shaking.
    await tester.pump(const Duration(milliseconds: 100));
    expect(find.byKey(const ValueKey('auth 4')), findsOneWidget);
    await tester.pumpAndSettle();
    final field = tester.widget<EditableText>(
      find.byKey(const ValueKey('auth password')),
    );
    expect(field.controller.text, isEmpty);
    expect(field.obscureText, isTrue);

    await tester.enterText(
      find.byKey(const ValueKey('auth password')),
      'secret',
    );
    await tester.testTextInput.receiveAction(TextInputAction.done);
    await tester.pumpAndSettle();
    expect(auth.answers.last, '4 anna secret');
    expect(find.byKey(const ValueKey('auth 4')), findsNothing);
  });

  testWidgets('Cancel, Escape and polkit withdrawing close the dialog', (
    tester,
  ) async {
    final auth = FakeAuth()
      ..waiting = [
        request,
        const AuthRequest(
          id: 5,
          action: 'x',
          message: '',
          app: '',
          users: {'anna': 'Anna'},
          defaultUser: 'anna',
        ),
      ];
    await pump(tester, auth);
    await tester.pump();
    expect(find.byKey(const ValueKey('auth 4')), findsOneWidget);
    await tester.tap(find.byKey(const ValueKey('auth cancel')));
    await tester.pump();
    expect(auth.cancelled, [4]);
    // The next one, from an unknown app: the generic title and the drawn padlock.
    expect(find.byKey(const ValueKey('auth 5')), findsOneWidget);
    expect(
      find.text('A system change needs an administrator’s permission.'),
      findsOneWidget,
    );
    auth.closes.add(5);
    await tester.pump();
    expect(find.byKey(const ValueKey('auth 5')), findsNothing);

    auth.requests.add(request);
    await tester.pump();
    await tester.pump();
    await tester.pump();
    await tester.sendKeyEvent(LogicalKeyboardKey.escape);
    await tester.pump();
    expect(auth.cancelled, [4, 4]);
    expect(find.byKey(const ValueKey('auth 4')), findsNothing);
  });

  testWidgets('a user polkit did not offer is not sent', (tester) async {
    final auth = FakeAuth();
    await pump(tester, auth);
    auth.requests.add(request);
    await tester.pump();
    await tester.pump();
    await tester.enterText(find.byKey(const ValueKey('auth user')), 'mallory');
    await tester.enterText(
      find.byKey(const ValueKey('auth password')),
      'secret',
    );
    await tester.tap(find.byKey(const ValueKey('auth allow')));
    await tester.pumpAndSettle();
    expect(auth.answers, isEmpty);
    expect(find.byKey(const ValueKey('auth 4')), findsOneWidget);
  });
}
