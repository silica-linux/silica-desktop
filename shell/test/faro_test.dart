import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:silica_shell/faro/faro.dart';
import 'package:silica_shell/faro/search.dart';
import 'package:silica_shell/system/desktop_entries.dart';
import 'package:silica_shell/system/shell_channel.dart';

const entries = {
  'term': DesktopEntry(
    id: 'term',
    name: 'Terminale',
    exec: 'terminale',
    keywords: ['shell'],
  ),
  'text': DesktopEntry(id: 'text', name: 'Editor di testo', exec: 'editor'),
  'calc': DesktopEntry(id: 'calc', name: 'Calcolatrice', exec: 'calc'),
  'tet': DesktopEntry(id: 'tet', name: 'Tetris', exec: 'tetris'),
};

void main() {
  test('la ricerca mette prima i nomi che iniziano con il testo', () {
    expect(
      [for (final e in searchApps(entries.values, 'te')) e.name],
      ['Terminale', 'Tetris', 'Editor di testo'],
    );
    expect(
      [for (final e in searchApps(entries.values, 'shell')) e.name],
      ['Terminale'],
    );
    expect(searchApps(entries.values, '  '), isEmpty);
    const twice = [
      DesktopEntry(id: 'a', name: 'Ufficio', exec: 'ufficio'),
      DesktopEntry(id: 'b', name: 'Ufficio', exec: 'ufficio'),
    ];
    expect(searchApps(twice, 'uff'), hasLength(1));
  });

  test("l'opacità dell'apertura segue i fotogrammi misurati", () {
    expect(opacityAt(0), 0.22);
    expect(opacityAt(13.3), closeTo(0.66, 0.001));
    expect(opacityAt(100), 1);
  });

  testWidgets('scrivere filtra, Invio avvia la prima app e chiude Faro', (
    tester,
  ) async {
    final calls = <MethodCall>[];
    tester.binding.defaultBinaryMessenger.setMockMethodCallHandler(
      ShellChannel.channel,
      (call) async {
        calls.add(call);
        return true;
      },
    );
    tester.view.physicalSize = const Size(1440, 900);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.reset);
    await tester.pumpWidget(
      const Directionality(
        textDirection: TextDirection.ltr,
        child: Faro(entries: entries, startOpen: true),
      ),
    );
    await tester.pumpAndSettle();
    expect(find.text('Cerca con Faro'), findsOneWidget);

    await tester.enterText(find.byType(EditableText), 'calc');
    await tester.pumpAndSettle();
    expect(find.text('Calcolatrice'), findsOneWidget);

    await tester.sendKeyEvent(LogicalKeyboardKey.enter);
    await tester.pumpAndSettle();
    final launch = calls.where((c) => c.method == 'launch').single;
    expect((launch.arguments as Map)['command'], 'calc');
    expect(find.text('Cerca con Faro'), findsNothing);
  });
}
