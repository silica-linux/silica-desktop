import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:silica_shell/faro/answers.dart';
import 'package:silica_shell/faro/faro.dart';
import 'package:silica_shell/faro/folders.dart';
import 'package:silica_shell/faro/search.dart';
import 'package:silica_shell/system/desktop_entries.dart';
import 'package:silica_shell/system/shell_channel.dart';

const entries = {
  'term': DesktopEntry(
    id: 'term',
    name: 'Terminal',
    exec: 'terminale',
    keywords: ['shell'],
  ),
  'text': DesktopEntry(id: 'text', name: 'Editor di testo', exec: 'editor'),
  'calc': DesktopEntry(id: 'calc', name: 'Calcolatrice', exec: 'calc'),
  'tet': DesktopEntry(id: 'tet', name: 'Tetris', exec: 'tetris'),
};

void main() {
  test('search puts first the names starting with the text', () {
    expect(
      [for (final e in searchApps(entries.values, 'te')) e.name],
      ['Terminal', 'Tetris', 'Editor di testo'],
    );
    expect(
      [for (final e in searchApps(entries.values, 'shell')) e.name],
      ['Terminal'],
    );
    expect(searchApps(entries.values, '  '), isEmpty);
    const twice = [
      DesktopEntry(id: 'a', name: 'Ufficio', exec: 'ufficio'),
      DesktopEntry(id: 'b', name: 'Ufficio', exec: 'ufficio'),
    ];
    expect(searchApps(twice, 'uff'), hasLength(1));
  });

  test('the opening opacity follows the measured frames', () {
    expect(opacityAt(0), 0.22);
    expect(opacityAt(13.3), closeTo(0.66, 0.001));
    expect(opacityAt(100), 1);
  });

  testWidgets('typing filters, Enter launches the first app and closes Faro', (
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
      Directionality(
        textDirection: TextDirection.ltr,
        child: Faro(
          entries: entries,
          startOpen: true,
          folders: FolderIndex.empty,
        ),
      ),
    );
    await tester.pumpAndSettle();
    expect(find.text('Search with Faro'), findsOneWidget);

    await tester.enterText(find.byType(EditableText), 'calc');
    await tester.pumpAndSettle();
    expect(find.text('Calcolatrice'), findsOneWidget);

    await tester.sendKeyEvent(LogicalKeyboardKey.enter);
    await tester.pumpAndSettle();
    final launch = calls.where((c) => c.method == 'launch').single;
    expect((launch.arguments as Map)['command'], 'calc');
    expect(find.text('Search with Faro'), findsNothing);
  });

  test(
    'calculations: operators, precedence, functions, a bare number is none',
    () {
      expect(calculate('12*3'), 36);
      expect(calculate('2+3*4'), 14);
      expect(calculate('(2+3)*4'), 20);
      expect(calculate('2^3^2'), 512);
      expect(calculate('-3 + 5'), 2);
      expect(calculate('1,5 * 2'), 3);
      expect(calculate('50%'), 0.5);
      expect(calculate('7 % 3'), 1);
      expect(calculate('sqrt 16'), 4);
      expect(calculate('2 * pi'), closeTo(6.2832, 1e-4));
      expect(calculate('12'), isNull);
      expect(calculate('e'), isNull);
      expect(calculate('pi'), isNull);
      expect(calculate('1/0'), isNull);
      expect(calculate('hello'), isNull);
      expect(calculate('3 +'), isNull);
      expect(calculate('(1+2'), isNull);
    },
  );

  test('unit conversions between units of the same quantity', () {
    final (miles, unit) = convertUnits('10 km in miglia')!;
    expect(miles, closeTo(6.2137, 1e-4));
    expect(unit, 'miglia');
    expect(convertUnits('100 °F to °C')!.$1, closeTo(37.78, 0.01));
    expect(convertUnits('2 kg in lb')!.$1, closeTo(4.409, 0.001));
    expect(convertUnits('10 km in kg'), isNull);
    expect(convertUnits('100 EUR in USD'), isNull);
  });

  test('answers are formatted with the language decimal separator', () {
    expect(
      answerFor('10 km in miglia', decimal: ',')!.result,
      '6,213711922 miglia',
    );
    expect(answerFor('12*3')!.result, '36');
    expect(answerFor('12*3')!.expression, '12*3 =');
    expect(answerFor('1/3')!.result, '0.3333333333');
    expect(answerFor('terminal'), isNull);
  });

  test('folder search ranks prefix, then word, then substring', () {
    final index = FolderIndex(root: '');
    final day = DateTime(2026, 10, 14);
    index.debugSetFolders([
      FolderHit(
        path: '/h/a/old-silica',
        name: 'old-silica',
        parent: 'a',
        modified: day,
      ),
      FolderHit(path: '/h/silica', name: 'silica', parent: 'h', modified: day),
      FolderHit(
        path: '/h/fossilica',
        name: 'fossilica',
        parent: 'h',
        modified: day,
      ),
      FolderHit(path: '/h/docs', name: 'docs', parent: 'h', modified: day),
    ]);
    expect(
      [for (final f in index.search('silica')) f.name],
      ['silica', 'old-silica', 'fossilica'],
    );
    expect(index.search('s'), isEmpty);
  });

  testWidgets('an answer card, the fallback rows and the arrow keys', (
    tester,
  ) async {
    tester.binding.defaultBinaryMessenger.setMockMethodCallHandler(
      ShellChannel.channel,
      (call) async => true,
    );
    tester.view.physicalSize = const Size(1440, 900);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.reset);
    await tester.pumpWidget(
      Directionality(
        textDirection: TextDirection.ltr,
        child: Faro(
          entries: entries,
          startOpen: true,
          folders: FolderIndex.empty,
        ),
      ),
    );
    await tester.pumpAndSettle();
    await tester.enterText(find.byType(EditableText), '12*3');
    await tester.pumpAndSettle();
    expect(find.text('12*3 ='), findsOneWidget);
    expect(find.text('= 36'), findsOneWidget);
    expect(find.text('Search the Web'), findsOneWidget);

    // With no app matching there is no message: only the fallback searches.
    await tester.enterText(find.byType(EditableText), 'zqxjvkwpf');
    await tester.pumpAndSettle();
    expect(find.text('— Search the Web'), findsNothing);
    expect(find.text('Search in Files'), findsOneWidget);
    await tester.sendKeyEvent(LogicalKeyboardKey.arrowDown);
    await tester.pumpAndSettle();
    expect(find.text('— Search the Web'), findsOneWidget);
  });
}
