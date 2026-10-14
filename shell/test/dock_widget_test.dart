import 'package:flutter/gestures.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:silica_shell/dock/dock.dart';
import 'package:silica_shell/dock/model.dart';
import 'package:silica_ui/silica_ui.dart';
import 'package:silica_shell/main.dart';
import 'package:silica_shell/system/layer_control.dart';

void main() {
  testWidgets(
    'il puntatore ingrandisce, mostra l\'etichetta e all\'uscita il Dock torna a riposo',
    (tester) async {
      tester.view.physicalSize = const Size(2560, 200);
      tester.view.devicePixelRatio = 1;
      addTearDown(tester.view.reset);
      await tester.pumpWidget(const ShellComponent(component: 'dock-demo'));

      final rest = layout(placeholderItems, width: 2560, height: 200);
      final mouse = await tester.createGesture(kind: PointerDeviceKind.mouse);
      await mouse.addPointer(location: const Offset(0, 0));
      await mouse.moveTo(Offset(rest.center(6), 155));
      // ~60 ms di entrata.
      await tester.pump(const Duration(milliseconds: 30));
      await tester.pump(const Duration(milliseconds: 40));
      expect(find.text('Foto'), findsOneWidget);

      // Isteresi: poco sopra la soglia d'ingresso ma ancora sull'icona ingrandita, resta.
      final zoneTop = 200 - bottomGap - backgroundHeight - activeZoneAbove;
      await mouse.moveTo(Offset(rest.center(6), zoneTop - 3));
      await tester.pump(const Duration(milliseconds: 30));
      expect(find.text('Foto'), findsOneWidget);

      // Uscendo l'etichetta resta ~40 ms, poi sfuma in ~250 ms.
      await mouse.moveTo(const Offset(1280, 10));
      await tester.pump(const Duration(milliseconds: 30));
      expect(find.text('Foto'), findsOneWidget);
      await tester.pump(const Duration(milliseconds: 100));
      await tester.pump(const Duration(milliseconds: 300));
      await tester.pump(const Duration(milliseconds: 50));
      expect(find.text('Foto'), findsNothing);
    },
  );

  test('il rimbalzo sale a 17 pt e torna a terra ogni ~0,7 s', () {
    expect(bounceHeight(0), 0);
    expect(bounceHeight(293), 17);
    expect(bounceHeight(693 + 293), closeTo(17, 0.01));
    expect(bounceHeight(693), 0);
  });

  testWidgets('clic destro e pressione lunga aprono il menu; il clic attiva', (
    tester,
  ) async {
    tester.view.physicalSize = const Size(1280, 300);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.reset);
    final log = <String>[];
    const items = [
      Item.app('Terminale', command: 'term'),
      Item.app('Note', command: 'note'),
    ];
    await tester.pumpWidget(
      Directionality(
        textDirection: TextDirection.ltr,
        child: Dock(
          items: items,
          onActivate: (item) => log.add('attiva ${item.label}'),
          menuFor: (item) => [
            MenuAction(
              'Apri ${item.label}',
              onSelected: () => log.add('menu ${item.label}'),
            ),
          ],
        ),
      ),
    );
    final rest = layout(items, width: 1280, height: 300);
    final icon = Offset(rest.center(0), 300 - bottomGap - backgroundHeight / 2);
    // Clic: attiva.
    await tester.tapAt(icon);
    await tester.pump(const Duration(milliseconds: 50));
    expect(log, ['attiva Terminale']);
    // Clic destro: il menu, con la punta sopra l'icona.
    await tester.tapAt(icon, buttons: kSecondaryButton);
    await tester.pump();
    expect(find.text('Apri Terminale'), findsOneWidget);
    final body = tester.getRect(find.byType(MenuPanel));
    expect(body.left, closeTo(rest.center(0) - 27, 0.5));
    // Clic fuori: si chiude senza scegliere.
    await tester.tapAt(const Offset(10, 10));
    await tester.pump(const Duration(milliseconds: 100));
    await tester.pump(const Duration(milliseconds: 200));
    expect(find.text('Apri Terminale'), findsNothing);
    // Pressione lunga sulla seconda icona: il menu compare dopo ~575 ms.
    final second = Offset(rest.center(1), icon.dy);
    final gesture = await tester.startGesture(second);
    await tester.pump(const Duration(milliseconds: 500));
    expect(find.text('Apri Note'), findsNothing);
    await tester.pump(const Duration(milliseconds: 100));
    expect(find.text('Apri Note'), findsOneWidget);
    // Trascinando sulla voce e rilasciando la si sceglie.
    await gesture.moveTo(tester.getCenter(find.text('Apri Note')));
    await gesture.up();
    await tester.pump(const Duration(milliseconds: 150));
    expect(log, ['attiva Terminale', 'menu Note']);
  });

  testWidgets('il sottomenu aperto entra nella regione d\'input', (
    tester,
  ) async {
    tester.view.physicalSize = const Size(1280, 300);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.reset);
    final regions = <List<Rect>>[];
    tester.binding.defaultBinaryMessenger.setMockMethodCallHandler(
      LayerControl.channel,
      (call) async {
        if (call.method == 'setInputRegion') {
          final rects = (call.arguments as Map)['rects'] as List?;
          regions.add([
            for (final r in rects ?? const [])
              Rect.fromLTWH(
                (r[0] as num).toDouble(),
                (r[1] as num).toDouble(),
                (r[2] as num).toDouble(),
                (r[3] as num).toDouble(),
              ),
          ]);
        }
        return null;
      },
    );
    addTearDown(
      () => tester.binding.defaultBinaryMessenger.setMockMethodCallHandler(
        LayerControl.channel,
        null,
      ),
    );
    final log = <String>[];
    const items = [Item.app('Note', command: 'note')];
    await tester.pumpWidget(
      Directionality(
        textDirection: TextDirection.ltr,
        child: Dock(
          items: items,
          menuFor: (item) => [
            MenuAction(
              'Opzioni',
              submenu: [
                MenuAction(
                  'Rimuovi dal Dock',
                  onSelected: () => log.add('via'),
                ),
              ],
            ),
          ],
        ),
      ),
    );
    final rest = layout(items, width: 1280, height: 300);
    final icon = Offset(rest.center(0), 300 - bottomGap - backgroundHeight / 2);
    await tester.tapAt(icon, buttons: kSecondaryButton);
    await tester.pump();
    final mouse = await tester.createGesture(kind: PointerDeviceKind.mouse);
    await mouse.addPointer(location: icon);
    await mouse.moveTo(tester.getCenter(find.text('Opzioni')));
    await tester.pump(const Duration(milliseconds: 350));
    await tester.pump();
    await tester.pump();
    final item = tester.getCenter(find.text('Rimuovi dal Dock'));
    expect(regions.last.any((r) => r.contains(item)), isTrue);
  });

  testWidgets(
    'con il menu aperto la Dock prende il puntatore su tutto lo schermo',
    (tester) async {
      tester.view.physicalSize = const Size(1280, 300);
      tester.view.devicePixelRatio = 1;
      addTearDown(tester.view.reset);
      final regions = <List<Object?>>[];
      tester.binding.defaultBinaryMessenger.setMockMethodCallHandler(
        LayerControl.channel,
        (call) async {
          if (call.method == 'outputSize')
            return {'width': 1280, 'height': 800};
          if (call.method == 'setInputRegion') {
            regions.add((call.arguments as Map)['rects'] as List<Object?>);
          }
          return null;
        },
      );
      addTearDown(
        () => tester.binding.defaultBinaryMessenger.setMockMethodCallHandler(
          LayerControl.channel,
          null,
        ),
      );
      const items = [Item.app('Note', command: 'note')];
      await tester.pumpWidget(
        Directionality(
          textDirection: TextDirection.ltr,
          child: Dock(items: items, menuFor: (item) => [MenuAction('Apri')]),
        ),
      );
      await tester.pump();
      final rest = layout(items, width: 1280, height: 300);
      final icon = Offset(
        rest.center(0),
        300 - bottomGap - backgroundHeight / 2,
      );
      await tester.tapAt(icon, buttons: kSecondaryButton);
      await tester.pump();
      await tester.pump();
      // Un solo rettangolo, alto quanto lo schermo: il clic sulla scrivania chiude il menu.
      expect(regions.last, [
        [0, 0, 1280, 800],
      ]);
    },
  );

  testWidgets(
    'trascinare riordina; in alto compare "Rimuovi" e l\'icona si toglie',
    (tester) async {
      tester.view.physicalSize = const Size(1280, 400);
      tester.view.devicePixelRatio = 1;
      addTearDown(tester.view.reset);
      final orders = <String>[];
      const items = [
        Item.app('Uno', command: 'uno', pinned: true),
        Item.app('Due', command: 'due', pinned: true),
        Item.app('Tre', command: 'tre', pinned: true),
        Item.separator(),
        Item.app('Cestino', command: 'cestino'),
      ];
      await tester.pumpWidget(
        Directionality(
          textDirection: TextDirection.ltr,
          child: Dock(
            items: items,
            onReorder: (order, dropped) => orders.add(
              '${[for (final i in order) i.label].join(' ')} | ${dropped?.label}',
            ),
          ),
        ),
      );
      final rest = layout(items, width: 1280, height: 400);
      final y = 400 - bottomGap - backgroundHeight / 2;
      // Uno dopo Due.
      var gesture = await tester.startGesture(Offset(rest.center(0), y));
      await gesture.moveBy(const Offset(10, 0));
      await tester.pump();
      await gesture.moveTo(Offset(rest.center(1) + 20, y));
      await tester.pump();
      await gesture.up();
      await tester.pump(const Duration(milliseconds: 300));
      expect(orders, ['Due Uno Tre | Uno']);
      // Tre in alto, oltre 130 pt sopra la presa: "Rimuovi", e al rilascio sparisce.
      gesture = await tester.startGesture(Offset(rest.center(2), y));
      await gesture.moveBy(const Offset(0, -10));
      await tester.pump();
      await gesture.moveTo(Offset(rest.center(2), y - 140));
      await tester.pump();
      expect(find.text('Rimuovi'), findsOneWidget);
      await gesture.up();
      await tester.pump(const Duration(milliseconds: 100));
      expect(orders.last, 'Uno Due | null');
      await tester.pump(const Duration(milliseconds: 300));
      await tester.pump(const Duration(milliseconds: 50));
      expect(find.text('Rimuovi'), findsNothing);
    },
  );

  testWidgets(
    'clic destro sul separatore: il suo menu; il clic sinistro niente',
    (tester) async {
      tester.view.physicalSize = const Size(1280, 300);
      tester.view.devicePixelRatio = 1;
      addTearDown(tester.view.reset);
      final log = <String>[];
      const items = [
        Item.app('Terminale', command: 'term'),
        Item.separator(),
        Item.app('Cestino', command: 'trash'),
      ];
      await tester.pumpWidget(
        Directionality(
          textDirection: TextDirection.ltr,
          child: Dock(
            items: items,
            onActivate: (item) => log.add('attiva ${item.label}'),
            menuFor: (item) => item.kind == ItemKind.separator
                ? const [MenuAction('Mostra le app recenti')]
                : const [],
          ),
        ),
      );
      final rest = layout(items, width: 1280, height: 300);
      final separator = Offset(
        rest.center(1),
        300 - bottomGap - backgroundHeight / 2,
      );
      await tester.tapAt(separator);
      await tester.pump(const Duration(milliseconds: 50));
      expect(log, isEmpty);
      expect(find.byType(MenuPanel), findsNothing);
      await tester.tapAt(separator, buttons: kSecondaryButton);
      await tester.pump();
      expect(find.text('Mostra le app recenti'), findsOneWidget);
    },
  );
}
