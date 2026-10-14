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
    'il ventaglio segue le misure: passo 64,6, lato che cresce, arco a destra',
    () {
      // Icona a x 1703,7, zona del Dock da y 1351 (dock-stacks.md, 10 file).
      Rect rect(int i) {
        final s = fanSlot(i);
        final c = const Offset(1703.7, 1351) + s.center;
        return Rect.fromCenter(center: c, width: s.side, height: s.side);
      }

      expect(rect(0).left, closeTo(1673.1, 0.2));
      expect(rect(0).top, closeTo(1278.1, 0.3));
      expect(rect(9).left, closeTo(1739.7, 0.3));
      expect(rect(9).top, closeTo(696.7, 1.5));
      expect(rect(15).left, closeTo(1850.9, 0.3));
      expect(fanSlot(15).side, closeTo(82.3, 0.2));
      expect(fanSlot(5).angle, greaterThan(fanSlot(1).angle));
    },
  );

  test('vista automatica e forma della griglia', () {
    expect(automaticView(10), StackView.fan);
    expect(automaticView(40), StackView.grid);
    expect(gridShape(3), (2, 2));
    expect(gridShape(10), (4, 3));
    expect(gridShape(40), (7, 6));
    expect(gridPanelSize(3, 2000).width, 290);
    expect(gridPanelSize(10, 2000).width, 546);
    // Superficie ancora bassa: niente eccezioni, la riga minima.
    expect(gridPanelSize(40, 71).height, 32 + 128 + 11);
  });

  test('ordine naturale: documento-2 prima di documento-10', () {
    final names = ['documento-10.txt', 'Documento-2.txt', 'documento-1.txt'];
    names.sort(naturalCompare);
    expect(names, ['documento-1.txt', 'Documento-2.txt', 'documento-10.txt']);
  });

  test('un percorso con apici resta un solo argomento', () {
    expect(shellQuote("/tmp/l'uno.txt"), "'/tmp/l'\\''uno.txt'");
  });

  testWidgets(
    'un clic sulla cartella apre il ventaglio; un clic su un file lo apre',
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
      final items = [Item.app('Pila', command: 'x', folder: dir.path)];
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
      // Per nome, senza i nascosti, e in cima "Apri la cartella".
      expect(find.text('a.txt'), findsOneWidget);
      expect(find.text('b.txt'), findsOneWidget);
      expect(find.text('.nascosto'), findsNothing);
      expect(find.text('Apri la cartella'), findsOneWidget);
      expect(
        tester.getCenter(find.text('a.txt')).dy,
        greaterThan(tester.getCenter(find.text('b.txt')).dy),
      );
      await tester.tap(find.text('b.txt'));
      // La pila si chiude in ~210 ms, su più fotogrammi.
      for (var i = 0; i < 6; i++) {
        await tester.pump(const Duration(milliseconds: 50));
      }
      expect(opened, ['${dir.path}/b.txt']);
      expect(find.text('a.txt'), findsNothing);
    },
  );

  testWidgets('con 40 file la pila è una griglia con il titolo', (
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
    // 7 colonne: 7 × 128 + 34 = 930 pt.
    final panel = tester.getRect(find.byType(LiquidGlass).last);
    expect(panel.width, closeTo(930, 1));
  });

  testWidgets('celle della griglia: icona 92 da y 10, nome su una riga', (
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
    // Una riga, troncata al centro: la fine del nome resta.
    expect(text.maxLines, 1);
    expect(text.data, contains('…'));
    expect(text.data, endsWith('.txt'));
    final icon = tester.getRect(
      find.byKey(
        ValueKey('icona ${dir.path}/documento con un nome molto lungo 1.txt'),
      ),
    );
    expect(icon.width, gridIconCanvas);
  });

  testWidgets('vista Elenco: un menu con i file, Opzioni e Apri', (
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
    final items = [Item.app('Pila', command: 'x', folder: dir.path)];
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
            const MenuAction('Opzioni'),
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
    expect(find.text('Opzioni'), findsOneWidget);
    // Nessun ventaglio.
    expect(find.text('Apri la cartella'), findsNothing);
  });

  testWidgets('mostrata come pila: l\'icona sono i primi tre file', (
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
        items: [Item.app('Pila', command: 'x', folder: dir.path)],
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

  test('ordine della pila: per data dal più recente, per tipo', () {
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

  test('impostazioni delle pile salvate per percorso', () {
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

  test('la cartella Download da xdg-user-dirs in XDG_CONFIG_HOME', () {
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
