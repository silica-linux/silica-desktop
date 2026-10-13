import 'package:flutter/gestures.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:silica_shell/dock/dock.dart';
import 'package:silica_shell/dock/model.dart';
import 'package:silica_ui/silica_ui.dart';
import 'package:silica_shell/main.dart';

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
}
