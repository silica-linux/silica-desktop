import 'dart:io';

import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:silica_shell/system/desktop_entries.dart';
import 'package:silica_shell/system/shell_icon.dart';
import 'package:silica_shell/system/silica_icons.dart';

const String svg =
    '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1024 1024">'
    '<rect x="100" y="100" width="824" height="824" fill="#2f8cff"/></svg>';

const String mailManifest = '''
{
  "format": 1, "id": "mail", "name": "Mail", "kind": "app", "keyline": "landscape",
  "layers": [
    { "role": "background", "file": "background.svg" },
    { "role": "foreground", "file": "foreground.svg", "shadow": "neutral" }
  ],
  "variants": { "dark": { "background": "background-dark.svg" } },
  "dynamic": null
}
''';

/// Una cartella di icone finta: `apps/<id>/` con i file indicati.
Directory iconTree(Map<String, Map<String, String>> icons) {
  final root = Directory.systemTemp.createTempSync('silica-icons-');
  for (final MapEntry(key: path, value: files) in icons.entries) {
    final dir = Directory('${root.path}/$path')..createSync(recursive: true);
    for (final MapEntry(key: name, value: text) in files.entries) {
      File('${dir.path}/$name').writeAsStringSync(text);
    }
  }
  return root;
}

void main() {
  test(
    'manifest valido: livelli in ordine, variante scura, ombra del primo piano',
    () {
      final root = iconTree({
        'apps/mail': {
          'icon.json': mailManifest,
          'background.svg': svg,
          'background-dark.svg': svg,
          'foreground.svg': svg,
        },
      });
      addTearDown(() => root.deleteSync(recursive: true));
      final icon = findSilicaIcon(
        'mail',
        env: {'SILICA_ICONS_DIR': root.path},
      )!;
      expect(icon.kind, IconKind.app);
      expect(icon.layers, hasLength(2));
      expect(
        icon.layers[0].pathFor(dark: true),
        endsWith('background-dark.svg'),
      );
      expect(icon.layers[1].pathFor(dark: true), endsWith('foreground.svg'));
      expect(icon.layers[1].shadow, isTrue);
    },
  );

  test(
    'manifest ostile: JSON rotto, file mancanti o fuori cartella sono rifiutati',
    () {
      final dir = Directory.systemTemp.createTempSync('silica-icon-');
      addTearDown(() => dir.deleteSync(recursive: true));
      File('${dir.path}/background.svg').writeAsStringSync(svg);
      expect(parseManifest(dir.path, '{'), isNull);
      expect(parseManifest(dir.path, '[]'), isNull);
      expect(parseManifest(dir.path, mailManifest), isNull);
      expect(
        parseManifest(
          dir.path,
          mailManifest.replaceAll('foreground.svg', '../../etc/passwd.svg'),
        ),
        isNull,
      );
      expect(
        parseManifest(
          dir.path,
          mailManifest.replaceAll('"format": 1', '"format": 2'),
        ),
        isNull,
      );
      expect(
        findSilicaIcon('../mail', env: {'SILICA_ICONS_DIR': dir.path}),
        isNull,
      );
    },
  );

  test('orologio e calendario dinamici', () {
    final root = iconTree({
      'apps/clock': {
        'icon.json': '''
{ "format": 1, "id": "clock", "name": "Clock", "kind": "app",
  "layers": [ { "role": "background", "file": "background.svg" } ],
  "dynamic": { "type": "clock", "cap": "cap.svg", "hands": [
    { "unit": "hour", "file": "hand-hour.svg" },
    { "unit": "minute", "file": "hand-minute.svg" },
    { "unit": "second", "file": "hand-second.svg", "min_size": 64 } ] } }''',
        'background.svg': svg,
        'cap.svg': svg,
        'hand-hour.svg': svg,
        'hand-hour-dark.svg': svg,
        'hand-minute.svg': svg,
        'hand-second.svg': svg,
      },
      'apps/calendar': {
        'icon.json': '''
{ "format": 1, "id": "calendar", "name": "Calendar", "kind": "app",
  "layers": [ { "role": "background", "file": "background.svg" } ],
  "dynamic": { "type": "calendar",
    "month": { "size": 92, "weight": 900, "color": "#f2382e", "dark_color": "#ff5248",
               "box": [100, 206, 824, 120], "case": "upper", "max_width": 560 },
    "day": { "size": 280, "weight": 700, "color": "#ffffff", "box": [100, 410, 824, 460] } } }''',
        'background.svg': svg,
      },
    });
    addTearDown(() => root.deleteSync(recursive: true));
    final env = {'SILICA_ICONS_DIR': root.path};
    final clock = findSilicaIcon('clock', env: env)!.dynamic! as ClockHands;
    expect(clock.hour.pathFor(dark: true), endsWith('hand-hour-dark.svg'));
    expect(clock.second, isNotNull);
    final calendar =
        findSilicaIcon('calendar', env: env)!.dynamic! as CalendarText;
    expect(calendar.month.upper, isTrue);
    expect(calendar.month.darkColor, const Color(0xFFFF5248));
    expect(calendar.day.box, const Rect.fromLTWH(100, 410, 824, 460));
  });

  test('l\'icona di un\'app: X-Silica-Icon, poi la tabella delle app note', () {
    final root = iconTree({
      for (final id in ['calculator', 'notes'])
        'apps/$id': {
          'icon.json': mailManifest.replaceAll('"mail"', '"$id"'),
          'background.svg': svg,
          'foreground.svg': svg,
        },
    });
    addTearDown(() => root.deleteSync(recursive: true));
    final env = {'SILICA_ICONS_DIR': root.path};
    final calc = parseDesktopEntry(
      'org.gnome.Calculator',
      '[Desktop Entry]\nType=Application\nName=Calcolatrice\nExec=gnome-calculator',
    )!;
    // Le icone delle app sono delle app di Silica: un'app di terzi tiene la sua.
    expect(silicaIconFor(calc, env: env), isNull);
    final own = parseDesktopEntry(
      'org.silica.Notes',
      '[Desktop Entry]\nType=Application\nName=Note\nExec=notes\nX-Silica-Icon=notes',
    )!;
    expect(own.silicaIcon, 'notes');
    expect(silicaIconFor(own, env: env)?.id, 'notes');
    final other = parseDesktopEntry(
      'firefox',
      '[Desktop Entry]\nType=Application\nName=Firefox\nExec=firefox\nIcon=firefox',
    )!;
    expect(silicaIconFor(other, env: env), isNull);
  });

  test(
    'la maschera del corpo sta nel rettangolo e non è un rettangolo arrotondato',
    () {
      const body = Rect.fromLTWH(0, 0, 100, 100);
      final path = bodyPath(body);
      expect(path.getBounds().left, closeTo(0, 0.01));
      expect(path.getBounds().right, closeTo(100, 0.01));
      expect(path.contains(const Offset(50, 0.5)), isTrue);
      expect(path.contains(const Offset(1, 1)), isFalse);
      // Superellisse n 2,6, r 32: sulla diagonale l'angolo è più pieno di un cerchio di r 32.
      expect(path.contains(const Offset(10, 10)), isTrue);
    },
  );

  testWidgets(
    'un\'icona a livelli si compone senza errori, anche con l\'orologio',
    (tester) async {
      final root = iconTree({
        'apps/mail': {
          'icon.json': mailManifest,
          'background.svg': svg,
          'background-dark.svg': svg,
          'foreground.svg': svg,
        },
        'apps/clock': {
          'icon.json': '''
{ "format": 1, "id": "clock", "name": "Clock", "kind": "app",
  "layers": [ { "role": "background", "file": "background.svg" } ],
  "dynamic": { "type": "clock", "cap": "cap.svg", "hands": [
    { "unit": "hour", "file": "hand.svg" }, { "unit": "minute", "file": "hand.svg" } ] } }''',
          'background.svg': svg,
          'cap.svg': svg,
          'hand.svg': svg,
        },
      });
      addTearDown(() => root.deleteSync(recursive: true));
      final env = {'SILICA_ICONS_DIR': root.path};
      await tester.pumpWidget(
        Directionality(
          textDirection: TextDirection.ltr,
          child: Row(
            children: [
              ShellIcon(
                LayeredSource(findSilicaIcon('mail', env: env)!),
                side: 48,
              ),
              ShellIcon(
                LayeredSource(findSilicaIcon('clock', env: env)!),
                side: 96,
                dark: true,
              ),
            ],
          ),
        ),
      );
      await tester.pump(const Duration(seconds: 2));
      expect(tester.takeException(), isNull);
      expect(find.byType(ShellIcon), findsNWidgets(2));
    },
  );
}
