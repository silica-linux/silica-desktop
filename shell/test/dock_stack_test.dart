import 'dart:io';

import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:silica_shell/dock/dock.dart';
import 'package:silica_shell/dock/model.dart';
import 'package:silica_shell/dock/stack.dart';
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
}
