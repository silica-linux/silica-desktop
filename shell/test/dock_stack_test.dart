import 'dart:io';

import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:silica_shell/dock/apps.dart' show downloadDir;
import 'package:silica_shell/dock/dock.dart';
import 'package:silica_shell/dock/model.dart';
import 'package:silica_shell/dock/stack.dart';
import 'package:silica_shell/dock/stack_settings.dart';
import 'package:silica_ui/silica_ui.dart';

void main() {
  setUp(() {
    TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
        .setMockMethodCallHandler(GlassRegistry.channel, (call) async => true);
  });

  test(
    'the fan follows the measurements (pitch and shift widened by fanSpread, provisional)',
    () {
      // Icon at x 1703.7, Dock area from y 1351 (dock-stacks.md, 10 files).
      Rect rect(int i) {
        final s = fanSlot(i);
        final c = const Offset(1703.7, 1351) + s.center;
        return Rect.fromCenter(center: c, width: s.side, height: s.side);
      }

      expect(rect(0).left, closeTo(1673.1, 0.2));
      expect(rect(0).top, closeTo(1278.1, 0.3));
      expect(rect(9).left, closeTo(1673.1 + 66.6 * fanSpread, 0.3));
      expect(rect(9).top, closeTo(1351 - 72.8 - 64.6 * 9 * fanSpread, 1.5));
      expect(rect(15).left, closeTo(1673.1 + 177.8 * fanSpread, 0.3));
      expect(fanSlot(15).side, closeTo(82.3, 0.2));
      expect(fanSlot(5).angle, greaterThan(fanSlot(1).angle));
    },
  );

  test('automatic view and grid shape', () {
    expect(automaticView(10), StackView.fan);
    expect(automaticView(40), StackView.grid);
    expect(gridShape(3), (2, 2));
    expect(gridShape(10), (4, 3));
    expect(gridShape(40), (7, 6));
    expect(gridPanelSize(3, 2000).width, 290);
    expect(gridPanelSize(10, 2000).width, 546);
    // Surface still low: no exceptions, the minimum row.
    expect(gridPanelSize(40, 71).height, 32 + 128 + 11);
  });

  test('natural order: documento-2 before documento-10', () {
    final names = ['documento-10.txt', 'Documento-2.txt', 'documento-1.txt'];
    names.sort(naturalCompare);
    expect(names, ['documento-1.txt', 'Documento-2.txt', 'documento-10.txt']);
  });

  test('a path with quotes stays a single argument', () {
    expect(shellQuote("/tmp/l'uno.txt"), "'/tmp/l'\\''uno.txt'");
  });

  testWidgets(
    'a click on the folder opens the fan; a click on a file opens it',
    (tester) async {
      final dir = Directory.systemTemp.createTempSync('pila');
      addTearDown(() => dir.deleteSync(recursive: true));
      for (final name in ['b.txt', 'a.txt', '.nascosto']) {
        File('${dir.path}/$name').writeAsStringSync('x');
      }
      tester.view.physicalSize = const Size(1280, 1000);
      tester.view.devicePixelRatio = 1;
      addTearDown(tester.view.reset);
      final opened = <String>[];
      final items = [Item.app('Stack', command: 'x', folder: dir.path)];
      await tester.pumpWidget(
        Directionality(
          textDirection: TextDirection.ltr,
          child: Dock(items: items, onOpenPath: opened.add),
        ),
      );
      final rest = layout(items, width: 1280, height: 1000);
      await tester.tapAt(
        Offset(rest.center(0), 1000 - bottomGap - backgroundHeight / 2),
      );
      await tester.pump();
      await tester.pump(const Duration(milliseconds: 300));
      // By name, without the hidden ones, and "Open folder" on top.
      expect(find.text('a.txt'), findsOneWidget);
      expect(find.text('b.txt'), findsOneWidget);
      expect(find.text('.nascosto'), findsNothing);
      expect(find.text('Open folder'), findsOneWidget);
      expect(
        tester.getCenter(find.text('a.txt')).dy,
        greaterThan(tester.getCenter(find.text('b.txt')).dy),
      );
      await tester.tap(find.text('b.txt'));
      // The stack closes in ~210 ms, over several frames.
      for (var i = 0; i < 6; i++) {
        await tester.pump(const Duration(milliseconds: 50));
      }
      expect(opened, ['${dir.path}/b.txt']);
      expect(find.text('a.txt'), findsNothing);
    },
  );

  testWidgets('with 40 files the stack is a grid with the title', (
    tester,
  ) async {
    final dir = Directory.systemTemp.createTempSync('griglia');
    addTearDown(() => dir.deleteSync(recursive: true));
    for (var i = 1; i <= 40; i++) {
      File('${dir.path}/documento-$i.txt').writeAsStringSync('x');
    }
    tester.view.physicalSize = const Size(2560, 1440);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.reset);
    final items = [Item.app('Download', command: 'x', folder: dir.path)];
    await tester.pumpWidget(
      Directionality(
        textDirection: TextDirection.ltr,
        child: Dock(items: items, onOpenPath: (_) {}),
      ),
    );
    final rest = layout(items, width: 2560, height: 1440);
    await tester.tapAt(
      Offset(rest.center(0), 1440 - bottomGap - backgroundHeight / 2),
    );
    for (var i = 0; i < 6; i++) {
      await tester.pump(const Duration(milliseconds: 60));
    }
    expect(find.text('documento-1.txt'), findsOneWidget);
    expect(find.text('Download'), findsOneWidget);
    // 7 columns: 7 × 128 + 34 = 930 pt.
    final panel = tester.getRect(
      find
          .descendant(
            of: find.byType(GridStack),
            matching: find.byType(LiquidGlass),
          )
          .first,
    );
    expect(panel.width, closeTo(930, 1));
  });

  testWidgets('grid cells: 92 icon from y 10, name on one line', (
    tester,
  ) async {
    final dir = Directory.systemTemp.createTempSync('griglia');
    addTearDown(() => dir.deleteSync(recursive: true));
    for (var i = 1; i <= 20; i++) {
      File(
        '${dir.path}/documento con un nome molto lungo $i.txt',
      ).writeAsStringSync('x');
    }
    tester.view.physicalSize = const Size(2560, 1440);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.reset);
    final items = [Item.app('Download', command: 'x', folder: dir.path)];
    await tester.pumpWidget(
      Directionality(
        textDirection: TextDirection.ltr,
        child: Dock(
          items: items,
          onOpenPath: (_) {},
          stackSettingsFor: (_) =>
              const StackSettings(content: StackContent.grid),
        ),
      ),
    );
    final rest = layout(items, width: 2560, height: 1440);
    await tester.tapAt(
      Offset(rest.center(0), 1440 - bottomGap - backgroundHeight / 2),
    );
    for (var i = 0; i < 6; i++) {
      await tester.pump(const Duration(milliseconds: 60));
    }
    final label = find.textContaining('document…');
    expect(label, findsWidgets);
    final text = tester.widget<Text>(label.first);
    // One line, truncated in the middle: the end of the name stays.
    expect(text.maxLines, 1);
    expect(text.data, contains('…'));
    expect(text.data, endsWith('.txt'));
    final icon = tester.getRect(
      find.byKey(
        ValueKey('icon ${dir.path}/documento con un nome molto lungo 1.txt'),
      ),
    );
    expect(icon.width, gridIconCanvas);
  });

  testWidgets('List view: a menu with the files, Options and Open', (
    tester,
  ) async {
    final dir = Directory.systemTemp.createTempSync('elenco');
    addTearDown(() => dir.deleteSync(recursive: true));
    for (final name in ['b.txt', 'a.txt']) {
      File('${dir.path}/$name').writeAsStringSync('x');
    }
    tester.view.physicalSize = const Size(1280, 1000);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.reset);
    final items = [Item.app('Stack', command: 'x', folder: dir.path)];
    await tester.pumpWidget(
      Directionality(
        textDirection: TextDirection.ltr,
        child: Dock(
          items: items,
          onOpenPath: (_) {},
          stackSettingsFor: (_) =>
              const StackSettings(content: StackContent.list),
          stackListMenu: (item, entries) => [
            for (final e in entries) MenuAction(fileName(e.path)),
            const MenuDivider(),
            const MenuAction('Options'),
          ],
        ),
      ),
    );
    final rest = layout(items, width: 1280, height: 1000);
    await tester.tapAt(
      Offset(rest.center(0), 1000 - bottomGap - backgroundHeight / 2),
    );
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 300));
    expect(find.byType(MenuPanel), findsOneWidget);
    expect(
      tester.getTopLeft(find.text('a.txt')).dy,
      lessThan(tester.getTopLeft(find.text('b.txt')).dy),
    );
    expect(find.text('Options'), findsOneWidget);
    // No fan.
    expect(find.text('Open folder'), findsNothing);
  });

  testWidgets('shown as a stack: the icon is the first three files', (
    tester,
  ) async {
    final dir = Directory.systemTemp.createTempSync('pila');
    addTearDown(() => dir.deleteSync(recursive: true));
    for (final name in ['a.txt', 'b.txt', 'c.txt', 'd.txt']) {
      File('${dir.path}/$name').writeAsStringSync('x');
    }
    tester.view.physicalSize = const Size(1280, 1000);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.reset);
    Widget dock(StackDisplay display) => Directionality(
      textDirection: TextDirection.ltr,
      child: Dock(
        items: [Item.app('Stack', command: 'x', folder: dir.path)],
        onOpenPath: (_) {},
        stackSettingsFor: (_) => StackSettings(display: display),
      ),
    );
    await tester.pumpWidget(dock(StackDisplay.stack));
    await tester.pump();
    final pile = find.byType(PileIcon);
    if (resolveFileIcon('${dir.path}/a.txt') != null) {
      expect(pile, findsOneWidget);
      expect(tester.widget<PileIcon>(pile).sources, hasLength(3));
    }
    await tester.pumpWidget(dock(StackDisplay.folder));
    await tester.pump();
    expect(find.byType(PileIcon), findsNothing);
  });

  test('stack order: by date from the most recent, by kind', () {
    final dir = Directory.systemTemp.createTempSync('ordine');
    addTearDown(() => dir.deleteSync(recursive: true));
    final old = File('${dir.path}/b.txt')..writeAsStringSync('x');
    old.setLastModifiedSync(DateTime(2020));
    File('${dir.path}/a.md').writeAsStringSync('x');
    File('${dir.path}/c.zip').writeAsStringSync('x');
    List<String> names(StackSort sort) => [
      for (final e in stackEntries(dir.path, sort: sort)) fileName(e.path),
    ];
    expect(names(StackSort.name), ['a.md', 'b.txt', 'c.zip']);
    expect(names(StackSort.modified).last, 'b.txt');
    expect(names(StackSort.kind), ['a.md', 'b.txt', 'c.zip']);
  });

  test('stack settings saved by path', () {
    final dir = Directory.systemTemp.createTempSync('config');
    addTearDown(() => dir.deleteSync(recursive: true));
    final env = {'XDG_CONFIG_HOME': dir.path, 'HOME': dir.path};
    expect(loadStackSettings(env: env), isEmpty);
    saveStackSettings({
      '/home/io/Download': const StackSettings(
        sort: StackSort.added,
        display: StackDisplay.folder,
        content: StackContent.list,
      ),
    }, env: env);
    final back = loadStackSettings(env: env)['/home/io/Download'];
    expect(back?.sort, StackSort.added);
    expect(back?.display, StackDisplay.folder);
    expect(back?.content, StackContent.list);
  });

  test('the Download folder from xdg-user-dirs in XDG_CONFIG_HOME', () {
    final dir = Directory.systemTemp.createTempSync('dirs');
    addTearDown(() => dir.deleteSync(recursive: true));
    Directory('${dir.path}/Scaricamenti').createSync();
    File('${dir.path}/cfg/user-dirs.dirs')
      ..createSync(recursive: true)
      ..writeAsStringSync('XDG_DOWNLOAD_DIR="\$HOME/Scaricamenti"\n');
    expect(
      downloadDir(dir.path, env: {'XDG_CONFIG_HOME': '${dir.path}/cfg'}),
      '${dir.path}/Scaricamenti',
    );
  });
}
