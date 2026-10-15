import 'package:flutter/gestures.dart';
import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:intl/date_symbol_data_local.dart';
import 'package:silica_shell/faro/answers.dart';
import 'package:silica_shell/faro/faro.dart';
import 'package:silica_shell/faro/rates.dart';
import 'package:silica_shell/faro/folders.dart';
import 'package:silica_shell/system/desktop_entries.dart';
import 'package:silica_shell/system/shell_channel.dart';

final entries = {
  'calc': const DesktopEntry(id: 'calc', name: 'Calculator', exec: 'calc'),
  'web': parseDesktopEntry('web', '''
[Desktop Entry]
Type=Application
Name=Browser
Exec=browser %U
Actions=new-window;private;

[Desktop Action new-window]
Name=New Window
Exec=browser --new-window

[Desktop Action private]
Name=New Private Window
Exec=browser --private
''')!,
};

FolderHit hit(String path, {bool folder = true}) => FolderHit(
  path: path,
  name: path.split('/').last,
  parent: path.split('/').reversed.skip(1).first,
  modified: DateTime(2026, 10, 14, 9),
  isFolder: folder,
);

void main() {
  setUpAll(() => initializeDateFormatting('en'));
  late List<MethodCall> calls;
  late FolderIndex folders;
  late SystemActions actions;
  late ClipboardHistory clipboard;

  Future<void> open(WidgetTester tester) async {
    calls = [];
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
    folders = FolderIndex(root: '')
      ..debugSetFolders([
        hit('/home/u/Projects'),
        hit('/home/u/Projects/Silica'),
        hit('/home/u/Projects/plan.md', folder: false),
        hit('/home/u/Documents/plan-old.md', folder: false),
      ]);
    actions = SystemActions()
      ..debugSet(const [
        SystemAction(
          id: 'screenshot.screen',
          label: 'Screenshot',
          shortcut: 'Logo+Shift+3',
        ),
        SystemAction(id: 'system.dark_toggle', label: 'Dark appearance'),
      ]);
    clipboard = ClipboardHistory()
      ..debugSet([
        ClipEntry(
          id: 2,
          kind: ClipKind.text,
          text: 'hello\nworld',
          time: DateTime.now(),
        ),
        ClipEntry(
          id: 1,
          kind: ClipKind.files,
          text: 'file:///home/u/a.txt\n',
          time: DateTime.now(),
        ),
      ]);
    await tester.pumpWidget(
      Directionality(
        textDirection: TextDirection.ltr,
        child: Faro(
          entries: entries,
          startOpen: true,
          folders: folders,
          actions: actions,
          clipboard: clipboard,
        ),
      ),
    );
    await tester.pumpAndSettle();
  }

  Future<void> hoverAndTap(WidgetTester tester, int button) async {
    final frame = tester.getRect(find.byKey(const ValueKey('faro panel')));
    final mouse = await tester.createGesture(kind: PointerDeviceKind.mouse);
    await mouse.addPointer(location: frame.center);
    await tester.pumpAndSettle();
    await tester.tap(find.byKey(ValueKey('faro button $button')));
    await tester.pumpAndSettle();
    await mouse.removePointer();
  }

  testWidgets('the round buttons switch scope; Esc goes back, then closes', (
    tester,
  ) async {
    await open(tester);
    await hoverAndTap(tester, 1);
    expect(find.byKey(const ValueKey('faro scope token')), findsOneWidget);
    expect(find.text('Files'), findsOneWidget);
    expect(find.text('Search Files'), findsOneWidget);
    await tester.sendKeyEvent(LogicalKeyboardKey.escape);
    await tester.pumpAndSettle();
    expect(find.byKey(const ValueKey('faro scope token')), findsNothing);
    expect(find.text('Search with Faro'), findsOneWidget);
    await tester.sendKeyEvent(LogicalKeyboardKey.escape);
    await tester.pumpAndSettle();
    expect(find.byKey(const ValueKey('faro panel')), findsNothing);
  });

  testWidgets(
    'files: files and folders; Return opens, Cmd+Return shows the folder',
    (tester) async {
      await open(tester);
      await tester.sendKeyDownEvent(LogicalKeyboardKey.metaLeft);
      await tester.sendKeyEvent(LogicalKeyboardKey.digit2);
      await tester.sendKeyUpEvent(LogicalKeyboardKey.metaLeft);
      await tester.pumpAndSettle();
      await tester.enterText(find.byType(EditableText), 'plan');
      await tester.pumpAndSettle();
      expect(find.text('plan.md'), findsOneWidget);
      expect(find.text('plan-old.md'), findsOneWidget);
      await tester.sendKeyDownEvent(LogicalKeyboardKey.metaLeft);
      await tester.sendKeyEvent(LogicalKeyboardKey.enter);
      await tester.sendKeyUpEvent(LogicalKeyboardKey.metaLeft);
      await tester.pumpAndSettle();
      final launch = calls.where((c) => c.method == 'launch').single;
      expect(
        (launch.arguments as Map)['command'],
        contains('/home/u/Projects'),
      );
    },
  );

  testWidgets('Tab on a folder searches inside it', (tester) async {
    await open(tester);
    await tester.enterText(find.byType(EditableText), 'proj');
    await tester.pumpAndSettle();
    expect(find.text('Projects'), findsOneWidget);
    await tester.sendKeyEvent(LogicalKeyboardKey.tab);
    await tester.pumpAndSettle();
    expect(find.text('Search in Projects'), findsOneWidget);
    // Inside it, with nothing typed: what it contains.
    expect(find.text('Silica'), findsOneWidget);
    expect(find.text('plan.md'), findsOneWidget);
    expect(find.text('plan-old.md'), findsNothing);
  });

  testWidgets('actions: the catalog and the apps\' actions, run on Return', (
    tester,
  ) async {
    await open(tester);
    await hoverAndTap(tester, 2);
    expect(find.text('Screenshot'), findsOneWidget);
    expect(find.text('⌘⇧3'), findsOneWidget);
    expect(find.text('New Private Window'), findsOneWidget);
    await tester.enterText(find.byType(EditableText), 'appearance');
    await tester.pumpAndSettle();
    await tester.sendKeyEvent(LogicalKeyboardKey.enter);
    await tester.pumpAndSettle();
    final execute = calls.where((c) => c.method == 'execute').single;
    expect((execute.arguments as Map)['id'], 'system.dark_toggle');
    expect(find.byKey(const ValueKey('faro panel')), findsNothing);
  });

  testWidgets('clipboard: the history, newest first, and Clear History', (
    tester,
  ) async {
    await open(tester);
    await hoverAndTap(tester, 3);
    expect(find.text('hello world'), findsOneWidget);
    expect(find.text('a.txt'), findsOneWidget);
    expect(find.text('Clear History'), findsOneWidget);
  });

  testWidgets(
    'Search in the file manager switches to the files scope with the query',
    (tester) async {
      await open(tester);
      await tester.enterText(find.byType(EditableText), 'nothing-like-this');
      await tester.pumpAndSettle();
      // The fallback rows: the first one searches the files.
      await tester.sendKeyEvent(LogicalKeyboardKey.enter);
      await tester.pumpAndSettle();
      expect(find.byKey(const ValueKey('faro scope token')), findsOneWidget);
      expect(calls.where((c) => c.method == 'launch'), isEmpty);
    },
  );

  Future<void> command(WidgetTester tester, LogicalKeyboardKey key) async {
    await tester.sendKeyDownEvent(LogicalKeyboardKey.metaLeft);
    await tester.sendKeyEvent(key);
    await tester.sendKeyUpEvent(LogicalKeyboardKey.metaLeft);
    await tester.pumpAndSettle();
  }

  Rect panelRect(WidgetTester tester) =>
      tester.getRect(find.byKey(const ValueKey('faro panel')));

  testWidgets('Cmd+1..4 open the scopes and switch straight between them', (
    tester,
  ) async {
    await open(tester);
    await command(tester, LogicalKeyboardKey.digit2);
    expect(find.text('Search Files'), findsOneWidget);
    await command(tester, LogicalKeyboardKey.digit3);
    expect(find.text('Search Actions'), findsOneWidget);
    await command(tester, LogicalKeyboardKey.digit4);
    expect(find.text('Search Clipboard'), findsOneWidget);
    await command(tester, LogicalKeyboardKey.digit2);
    expect(find.text('Search Files'), findsOneWidget);
    expect(find.byKey(const ValueKey('faro scope token')), findsOneWidget);
    // Cmd+1 is the Applications disc: the Launcher; from there Cmd+3 goes to Actions.
    await command(tester, LogicalKeyboardKey.digit1);
    expect(find.byKey(const ValueKey('faro scope token')), findsNothing);
    expect(find.text('Search Files'), findsNothing);
    await command(tester, LogicalKeyboardKey.digit3);
    expect(find.text('Search Actions'), findsOneWidget);
  });

  testWidgets('files: 640 x 547 with a grid of 5 columns', (tester) async {
    await open(tester);
    await command(tester, LogicalKeyboardKey.digit2);
    final rect = panelRect(tester);
    expect(rect.width, closeTo(640, 0.5));
    expect(rect.height, closeTo(547, 0.5));
    final grid = tester.widget<GridView>(
      find.byKey(const ValueKey('faro files grid')),
    );
    final delegate =
        grid.gridDelegate as SliverGridDelegateWithFixedCrossAxisCount;
    expect(delegate.crossAxisCount, 5);
    // Nothing typed: the recent files and folders fill the grid, side by side.
    expect(find.text('plan.md'), findsOneWidget);
    final first = tester.getCenter(find.text('Projects'));
    final second = tester.getCenter(find.text('Silica'));
    expect(second.dy, first.dy);
    expect(second.dx - first.dx, closeTo(124, 0.5));
  });

  testWidgets('Esc in a scope clears the text, then leaves the scope', (
    tester,
  ) async {
    await open(tester);
    await command(tester, LogicalKeyboardKey.digit3);
    await tester.enterText(find.byType(EditableText), 'shot');
    await tester.pumpAndSettle();
    await tester.sendKeyEvent(LogicalKeyboardKey.escape);
    await tester.pumpAndSettle();
    expect(find.byKey(const ValueKey('faro scope token')), findsOneWidget);
    expect(
      tester.widget<EditableText>(find.byType(EditableText)).controller.text,
      isEmpty,
    );
    await tester.sendKeyEvent(LogicalKeyboardKey.escape);
    await tester.pumpAndSettle();
    expect(find.byKey(const ValueKey('faro scope token')), findsNothing);
    expect(find.byKey(const ValueKey('faro panel')), findsOneWidget);
  });

  testWidgets(
    'the scope persists across close and reopen; closing takes 100 ms',
    (tester) async {
      await open(tester);
      await command(tester, LogicalKeyboardKey.digit3);
      await tester.tapAt(const Offset(5, 5));
      await tester.pump();
      await tester.pump(const Duration(milliseconds: 60));
      expect(find.byKey(const ValueKey('faro panel')), findsOneWidget);
      await tester.pump(const Duration(milliseconds: 60));
      expect(find.byKey(const ValueKey('faro panel')), findsNothing);
      (tester.state(find.byType(Faro)) as dynamic).reopenForTest();
      await tester.pumpAndSettle();
      expect(find.byKey(const ValueKey('faro scope token')), findsOneWidget);
      expect(find.text('Screenshot'), findsOneWidget);
    },
  );

  testWidgets('actions: sections per app, sorted alphabetically', (
    tester,
  ) async {
    await open(tester);
    await command(tester, LogicalKeyboardKey.digit3);
    double at(String text) => tester.getCenter(find.text(text)).dy;
    double section(String name) =>
        tester.getCenter(find.byKey(ValueKey('faro section $name'))).dy;
    final browser = section('Browser');
    final newPrivate = at('New Private Window');
    final newWindow = at('New Window');
    final system = section('System');
    final toggle = at('Toggle appearance');
    final shot = at('Screenshot');
    // Sections alphabetical (Browser before System); inside them, by name.
    expect(browser, lessThan(newPrivate));
    expect(newPrivate, lessThan(newWindow));
    expect(newWindow, lessThan(system));
    expect(system, lessThan(shot));
    expect(shot, lessThan(toggle));
  });

  testWidgets('a pill inserts the files token and keeps the typed text', (
    tester,
  ) async {
    await open(tester);
    await tester.enterText(find.byType(EditableText), 'plan');
    await tester.pumpAndSettle();
    await tester.tap(find.text('Files').first);
    await tester.pumpAndSettle();
    expect(find.byKey(const ValueKey('faro scope token')), findsOneWidget);
    expect(
      tester.widget<EditableText>(find.byType(EditableText)).controller.text,
      'plan',
    );
  });

  test('desktop actions are read from the entry', () {
    final web = entries['web']!;
    expect(
      [for (final a in web.actions) a.name],
      ['New Window', 'New Private Window'],
    );
    expect(web.actions.last.exec, 'browser --private');
  });

  test('shortcut glyphs', () {
    expect(shortcutGlyphs('Logo+Shift+3'), '⌘⇧3');
    expect(shortcutGlyphs('Logo+Alt+Escape'), '⌘⌥⎋');
  });

  test('currencies: codes, symbols and names, with the ECB rates', () {
    const xml = """<gesmes:Envelope><Cube><Cube time='2026-10-14'>
      <Cube currency='USD' rate='1.0800'/><Cube currency='GBP' rate='0.8500'/>
    </Cube></Cube></gesmes:Envelope>""";
    final parsed = parseEcbRates(xml)!;
    expect(parsed.$1, {'EUR': 1, 'USD': 1.08, 'GBP': 0.85});
    expect(parsed.$2, DateTime(2026, 10, 14));
    final rates = parsed.$1;
    final usd = convertCurrency('100 EUR in USD', rates)!;
    expect(usd.$1, closeTo(108, 1e-9));
    expect(usd.$2, 'USD');
    expect(convertCurrency('€20 to dollars', rates)!.$1, closeTo(21.6, 1e-9));
    expect(convertCurrency('108 usd in eur', rates)!.$1, closeTo(100, 1e-9));
    expect(convertCurrency('10 km in miles', rates), isNull);
    expect(convertCurrency('100 EUR in USD', const {}), isNull);
    expect(answerFor('100 EUR in GBP', rates: rates)!.result, '85.00 GBP');
    // Units still work next to currencies.
    expect(
      answerFor('10 km in miles', rates: rates)!.result,
      startsWith('6.2'),
    );
  });
}
