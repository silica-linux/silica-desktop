import 'dart:io';

import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:silica_shell/faro/launcher.dart';
import 'package:silica_shell/system/desktop_entries.dart';

DesktopEntry app(
  String id,
  String name, [
  List<String> categories = const [],
]) => DesktopEntry(id: id, name: name, exec: id, categories: categories);

final entries = {
  for (final e in [
    app('calc', 'Calcolatrice', ['Utility', 'Calculator']),
    app('cal', 'Calendario', ['Office', 'Calendar']),
    app('term', 'Terminale', ['System', 'TerminalEmulator']),
    app('gimp', 'Disegno', ['Graphics']),
    app('chess', 'Scacchi', ['Game']),
    app('mail', 'Posta', ['Network', 'Email']),
    app('docs', 'Lettore', ['Viewer']),
    app('odd', 'Strana'),
    for (var i = 0; i < 20; i++) app('x$i', 'App $i', ['Utility']),
  ])
    e.id: e,
};

void main() {
  test('le categorie freedesktop finiscono nelle sette del launcher', () {
    expect(categoryOf(entries['calc']!), 'Utility');
    expect(categoryOf(entries['term']!), 'Utility');
    expect(categoryOf(entries['cal']!), 'Produttività e finanza');
    expect(categoryOf(entries['gimp']!), 'Creatività');
    expect(categoryOf(entries['chess']!), 'Svago');
    expect(categoryOf(entries['mail']!), 'Social');
    expect(categoryOf(entries['docs']!), 'Informazione e lettura');
    expect(categoryOf(entries['odd']!), 'Altro');
    expect(categoryNames, hasLength(7));
  });

  test(
    'ricerca: su tutte le app la migliore prima, nella categoria in ordine alfabetico',
    () {
      final all = launcherApps(entries.values, query: 'cal');
      expect(
        [for (final e in all.take(2)) e.name],
        ['Calcolatrice', 'Calendario'],
      );
      final utility = launcherApps(entries.values, category: 'Utility');
      expect(utility.first.name, 'App 0');
      expect(utility.every((e) => categoryOf(e) == 'Utility'), isTrue);
      expect(
        [
          for (final e in launcherApps(
            entries.values,
            category: 'Utility',
            query: 'cal',
          ))
            e.name,
        ],
        ['Calcolatrice'],
      );
    },
  );

  test('griglia: recenti, linea e elenco alle distanze misurate', () {
    const layout = GridLayout(recents: 7, apps: 30);
    // Area da y 536: recenti a 547, linea a 652, elenco a 664, 766 (passo 102).
    expect(layout.recentCell(2), const Offset(20 + 2 * 112.5, 11));
    expect(layout.separatorY, 652 - 536);
    expect(layout.cell(0).dy, 664 - 536);
    expect(layout.cell(7).dy, 766 - 536);
    expect(layout.cell(8).dx, 20 + 112.5);
    // Senza recenti la prima cella sta a 11 pt dall'inizio (categoria: y 500 con area da 489).
    expect(const GridLayout(recents: 0, apps: 3).cell(0).dy, 11);
  });

  test(
    'barra di scorrimento: cursore proporzionale, nessuno se il contenuto entra',
    () {
      const bar = LauncherScrollbar(visible: 473, content: 1255, offset: 0);
      expect(bar.thumb(385)!.height, closeTo(145, 1));
      const scrolled = LauncherScrollbar(
        visible: 473,
        content: 1255,
        offset: 100,
      );
      expect(scrolled.thumb(385)!.top, closeTo(31, 1.5));
      expect(
        const LauncherScrollbar(
          visible: 520,
          content: 300,
          offset: 0,
        ).thumb(431),
        isNull,
      );
    },
  );

  test('le app recenti si ricordano dalla più recente, al più sette', () {
    final dir = Directory.systemTemp.createTempSync('recenti');
    addTearDown(() => dir.deleteSync(recursive: true));
    final recents = RecentApps(env: {'HOME': dir.path});
    expect(recents.load(), isEmpty);
    for (var i = 0; i < 9; i++) {
      recents.add('a$i');
    }
    recents.add('a3');
    expect(recents.load(), ['a3', 'a8', 'a7', 'a6', 'a5', 'a4', 'a2']);
  });

  testWidgets(
    'capsule larghe almeno 85,5 pt a 8 pt fra loro; la categoria diventa un gettone',
    (tester) async {
      final key = GlobalKey<LauncherState>();
      await tester.pumpWidget(
        Directionality(
          textDirection: TextDirection.ltr,
          child: Align(
            alignment: Alignment.topLeft,
            child: Launcher(
              key: key,
              entries: entries,
              recents: const ['term', 'calc'],
              onLaunch: (_) {},
              onBack: () {},
            ),
          ),
        ),
      );
      // Nei test il carattere è Ahem: si controllano margini e spazi, non le larghezze dei testi.
      Rect capsule(String name) => tester.getRect(
        find
            .ancestor(of: find.text(name), matching: find.byType(Container))
            .first,
      );
      final first = capsule('Produttività e finanza');
      final utility = capsule('Utility');
      // Con Ahem la riga è più larga del pannello e si stringe di k.
      final k = first.height / 26;
      expect(first.left, 20);
      expect(first.center.dy, closeTo(56 + 1 + 10 + 13, 0.01));
      expect(utility.left - first.right, closeTo(8 * k, 0.01));
      expect(utility.width, greaterThanOrEqualTo(85.5 * k));
      expect(find.text('Applicazioni'), findsOneWidget);
      await tester.tap(find.text('Utility'));
      await tester.pump(const Duration(milliseconds: 300));
      expect(key.currentState!.category, 'Utility');
      expect(find.text('Applicazioni'), findsNothing);
      // Le capsule spariscono, il gettone resta.
      expect(find.text('Social'), findsNothing);
      expect(find.text('Utility'), findsOneWidget);
    },
  );
}
