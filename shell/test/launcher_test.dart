import 'dart:io';

import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:silica_shell/faro/faro.dart' show backProgress;
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
    app('cal', 'Calendar', ['Office', 'Calendar']),
    app('term', 'Terminal', ['System', 'TerminalEmulator']),
    app('gimp', 'Disegno', ['Graphics']),
    app('chess', 'Scacchi', ['Game']),
    app('mail', 'Mail', ['Network', 'Email']),
    app('docs', 'Lettore', ['Viewer']),
    app('odd', 'Strana'),
    for (var i = 0; i < 20; i++) app('x$i', 'App $i', ['Utility']),
  ])
    e.id: e,
};

void main() {
  test('freedesktop categories end up in the seven of the launcher', () {
    expect(categoryOf(entries['calc']!), 'utilities');
    expect(categoryOf(entries['term']!), 'utilities');
    expect(categoryOf(entries['cal']!), 'productivity');
    expect(categoryOf(entries['gimp']!), 'creativity');
    expect(categoryOf(entries['chess']!), 'entertainment');
    expect(categoryOf(entries['mail']!), 'social');
    expect(categoryOf(entries['docs']!), 'reading');
    expect(categoryOf(entries['odd']!), 'other');
    expect(categoryNames, hasLength(7));
  });

  test(
    'search: over all apps the best first, in the category alphabetically',
    () {
      final all = launcherApps(entries.values, query: 'cal');
      expect(
        [for (final e in all.take(2)) e.name],
        ['Calcolatrice', 'Calendar'],
      );
      final utility = launcherApps(entries.values, category: 'utilities');
      expect(utility.first.name, 'App 0');
      expect(utility.every((e) => categoryOf(e) == 'utilities'), isTrue);
      expect(
        [
          for (final e in launcherApps(
            entries.values,
            category: 'utilities',
            query: 'cal',
          ))
            e.name,
        ],
        ['Calcolatrice'],
      );
    },
  );

  test('the file manager is in the grid, like Finder in Applications', () {
    const manager = DesktopEntry(
      id: fileManagerId,
      name: 'Mustache',
      exec: 'mustache',
      categories: ['System', 'FileManager'],
    );
    final all = launcherApps([...entries.values, manager]);
    expect(all.where((e) => e.id == fileManagerId), hasLength(1));
    expect(launcherApps([manager], query: 'must'), [manager]);
  });

  test('grid: recents, line and list at the measured distances', () {
    const layout = GridLayout(recents: 7, apps: 30);
    // Area from y 536: recents at 547, line at 652, list at 664, 766 (102 pitch).
    expect(layout.recentCell(2), const Offset(20 + 2 * 112.5, 11));
    expect(layout.separatorY, 652 - 536);
    expect(layout.cell(0).dy, 664 - 536);
    expect(layout.cell(7).dy, 766 - 536);
    expect(layout.cell(8).dx, 20 + 112.5);
    // Without recents the first cell is 11 pt from the start (category: y 500 with the area from 489).
    expect(const GridLayout(recents: 0, apps: 3).cell(0).dy, 11);
  });

  test('scroll bar: proportional thumb, none if the content fits', () {
    const bar = LauncherScrollbar(visible: 473, content: 1255, offset: 0);
    expect(bar.thumb(385)!.height, closeTo(145, 1));
    const scrolled = LauncherScrollbar(
      visible: 473,
      content: 1255,
      offset: 100,
    );
    expect(scrolled.thumb(385)!.top, closeTo(31, 1.5));
    expect(
      const LauncherScrollbar(visible: 520, content: 300, offset: 0).thumb(431),
      isNull,
    );
  });

  test('recent apps are remembered from the most recent, at most seven', () {
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
    'capsules at least 85.5 pt wide 8 pt apart; the category becomes a token',
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
      // In the tests the font is Ahem: margins and gaps are checked, not text widths.
      Rect capsule(String name) => tester.getRect(
        find
            .ancestor(of: find.text(name), matching: find.byType(Container))
            .first,
      );
      final first = capsule('Productivity and finance');
      final utility = capsule('Utilities');
      // With Ahem the row is wider than the panel and shrinks by k.
      final k = first.height / 26;
      expect(first.left, 20);
      expect(first.center.dy, closeTo(56 + 1 + 10 + 13, 0.01));
      expect(utility.left - first.right, closeTo(8 * k, 0.01));
      expect(utility.width, greaterThanOrEqualTo(85.5 * k));
      expect(find.text('Applications'), findsOneWidget);
      await tester.tap(find.text('Utilities'));
      await tester.pump(const Duration(milliseconds: 300));
      expect(key.currentState!.category, 'utilities');
      expect(find.text('Applications'), findsNothing);
      // The capsules disappear, the token stays.
      expect(find.text('Social'), findsNothing);
      expect(find.text('Utilities'), findsOneWidget);
    },
  );

  test('inline completion: only if the first result starts with the text', () {
    final results = launcherApps(entries.values, query: 'cal');
    expect(completionFor('cal', results), 'colatrice');
    expect(completionFor('Cal', results), 'colatrice');
    expect(
      completionFor('lat', launcherApps(entries.values, query: 'lat')),
      isNull,
    );
    expect(completionFor('', results), isNull);
  });

  test('the return to Faro follows the measured height', () {
    expect(backProgress(0), 0);
    // 577 → 234 pt at 106.7 ms: 343 / 520.
    expect(backProgress(106.7), closeTo(343 / 520, 0.001));
    expect(backProgress(300), 1);
  });

  testWidgets(
    'typing completes the name and puts the ring on the first result',
    (tester) async {
      tester.view.physicalSize = const Size(1200, 800);
      tester.view.devicePixelRatio = 1;
      addTearDown(tester.view.reset);
      await tester.pumpWidget(
        Directionality(
          textDirection: TextDirection.ltr,
          child: Align(
            alignment: Alignment.topLeft,
            child: Launcher(
              entries: entries,
              recents: const [],
              onLaunch: (_) {},
              onBack: () {},
            ),
          ),
        ),
      );
      await tester.enterText(find.byType(EditableText), 'cal');
      await tester.pump();
      expect(find.text('colatrice — Open'), findsOneWidget);
      // The ring: a 4 pt border around the icon of the first result.
      final ring = find.byWidgetPredicate(
        (w) =>
            w is DecoratedBox &&
            w.decoration is BoxDecoration &&
            (w.decoration as BoxDecoration).border != null,
      );
      expect(ring, findsOneWidget);
      expect(tester.getSize(ring), const Size(69, 69));
    },
  );
}
