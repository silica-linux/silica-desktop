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

const String symbolSvg =
    '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24">'
    '<path d="M4 4 L20 20" stroke="currentColor" stroke-width="1.5"/></svg>';

const String folderManifest = '''
{
  "format": 1, "id": "folder-generic", "name": "Folder Generic", "kind": "file",
  "layers": [ { "role": "artwork", "file": "icon.svg" } ],
  "variants": { "dark": { "artwork": "icon-dark.svg" } },
  "dynamic": {
    "type": "folder-symbol",
    "symbol": { "box": [374, 456, 276, 276], "sizes": [
      { "min_size": 16, "weight": "semibold", "scale": 1.272, "highlight": false },
      { "min_size": 128, "weight": "light", "scale": 1 },
      { "min_size": 64, "weight": "regular", "scale": 1 } ] },
    "effect": {
      "light": { "color": "#1a64c8", "opacity": 0.78, "highlight": "#ffffff",
                 "highlight_opacity": 0.5, "offset": 4 },
      "dark": { "color": "#0b3f94", "opacity": 0.8 } },
    "tint": {
      "light": { "luminance": [0.35, 0.85], "shade": 0.72, "glow": 0.5 },
      "labels": { "green": "#2fb350", "BAD": "#000000", "red": "rosso" } },
    "folders": { "HOME": "house", "DOCUMENTS": "doc-text", "DOWNLOAD": "missing",
                 "lower": "house", "MUSIC": "../etc/passwd" },
    "fallback": "folder"
  }
}
''';

/// Il repo finto per la cartella generica: cartella semplice, generica e due simboli.
Directory folderTree() => iconTree({
  'files/folder': {
    'icon.json': folderManifest
        .replaceAll('"folder-generic"', '"folder"')
        .replaceAll(
          RegExp(r'"dynamic": \{[\s\S]*\}\s*\}\s*$'),
          '"dynamic": null }',
        ),
    'icon.svg': svg,
  },
  'files/folder-generic': {
    'icon.json': folderManifest,
    'icon.svg': svg,
    'icon-dark.svg': svg,
  },
  for (final weight in ['light', 'regular', 'semibold'])
    'symbols/$weight': {'house.svg': symbolSvg, 'doc-text.svg': symbolSvg},
});

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

  test('cartella generica: fasce, effetto, tinta, mappa ripulita', () {
    final root = folderTree();
    addTearDown(() => root.deleteSync(recursive: true));
    final env = {'SILICA_ICONS_DIR': root.path, 'HOME': '/nessuna'};
    final plain = findSilicaIcon('folder', env: env);
    expect(plain, isNotNull);
    expect(plain!.dynamic, isNull);
    final icon = findSilicaIcon('folder-generic', env: env)!;
    final dyn = icon.dynamic! as FolderSymbol;
    expect(dyn.box, const Rect.fromLTWH(374, 456, 276, 276));
    // Fasce ordinate dalla più grande; sotto 16 niente simbolo.
    expect(dyn.bandFor(512)!.weight, 'light');
    expect(dyn.bandFor(96)!.weight, 'regular');
    expect(dyn.bandFor(32)!.scale, closeTo(1.272, 1e-9));
    expect(dyn.bandFor(32)!.highlight, isFalse);
    expect(dyn.bandFor(12), isNull);
    expect(dyn.light.offset, 4);
    // Il tema scuro senza riflesso: opacità 0.
    expect(dyn.dark.highlightOpacity, 0);
    expect(dyn.darkTint, same(dyn.lightTint));
    expect(dyn.labels.keys, ['green']);
    expect(dyn.folders, {
      'HOME': 'house',
      'DOCUMENTS': 'doc-text',
      'DOWNLOAD': 'missing',
    });
    expect(
      dyn.symbolFile('house', 'light'),
      endsWith('symbols/light/house.svg'),
    );
    expect(dyn.symbolFile('missing', 'light'), isNull);
    expect(dyn.symbolFile('../house', 'light'), isNull);
    expect(dyn.symbolFile('house', 'bold'), isNull);
    expect(dyn.tintFor('green'), const Color(0xFF2FB350));
    expect(dyn.tintFor('#e8336d'), const Color(0xFFE8336D));
    expect(dyn.tintFor('rosa'), isNull);
    // La rampa: tinta × shade a luminance[0], tinta schiarita a luminance[1].
    final m = dyn.lightTint!.matrix(const Color(0xFF000000 | 0xE8336D));
    double red(double l) =>
        (m[0] + m[1] + m[2]) * l + m[4] / 255; // grigio di luminanza l
    expect(red(0.35), closeTo(0xE8 / 255 * 0.72, 1e-6));
    expect(red(0.85), closeTo(0xE8 / 255 + (1 - 0xE8 / 255) * 0.5, 1e-6));
    expect(m.sublist(15), [0, 0, 0, 1, 0]);
  });

  test(
    'cartella generica ostile: le parti rotte la rifiutano o si scartano',
    () {
      final dir = Directory.systemTemp.createTempSync('silica-folder-');
      addTearDown(() => dir.deleteSync(recursive: true));
      File('${dir.path}/icon.svg').writeAsStringSync(svg);
      File('${dir.path}/icon-dark.svg').writeAsStringSync(svg);
      FolderSymbol? parse(String text) =>
          parseManifest(dir.path, text)?.dynamic as FolderSymbol?;
      expect(parse(folderManifest), isNotNull);
      for (final (from, to) in [
        ('"box": [374, 456, 276, 276]', '"box": [374, 456, 276]'),
        ('"box": [374, 456, 276, 276]', '"box": [374, 456, -1, 276]'),
        ('"box": [374, 456, 276, 276]', '"box": [374, 456, 1e309, 276]'),
        ('"weight": "light"', '"weight": "../light"'),
        ('"min_size": 128', '"min_size": "128"'),
        ('"scale": 1.272', '"scale": 1000'),
        ('"color": "#1a64c8"', '"color": "blu"'),
        ('"opacity": 0.78', '"opacity": 7.8'),
        ('"sizes": [', '"sizes": 3, "x": ['),
      ]) {
        expect(
          parseManifest(
            dir.path,
            folderManifest.replaceFirst(from, to),
          )?.dynamic,
          isNull,
          reason: to,
        );
      }
      // Una rampa piatta o fuori intervallo toglie solo la tinta.
      final flat = parse(
        folderManifest.replaceFirst('[0.35, 0.85]', '[0.5, 0.5]'),
      );
      expect(flat!.lightTint, isNull);
      final noFallback = parse(
        folderManifest.replaceFirst(
          '"fallback": "folder"',
          '"fallback": "/etc"',
        ),
      );
      expect(noFallback!.fallback, isNull);
    },
  );

  test('cartelle speciali: xdg-user-dirs, home, app, libreria', () {
    final home = Directory.systemTemp.createTempSync('silica-home-');
    addTearDown(() => home.deleteSync(recursive: true));
    Directory('${home.path}/.config').createSync();
    File('${home.path}/.config/user-dirs.dirs').writeAsStringSync(
      '# commento\n'
      'XDG_DESKTOP_DIR="\$HOME/Scrivania"\n'
      'XDG_DOCUMENTS_DIR="\$HOME/Documenti/"\n'
      'XDG_DOWNLOAD_DIR="/srv/scaricati"\n'
      'XDG_MUSIC_DIR="\$HOME/"\n'
      'XDG_PICTURES_DIR="relativo/Immagini"\n'
      'XDG_VIDEOS_DIR=senza-virgolette\n',
    );
    final env = {'HOME': home.path, 'XDG_DATA_DIRS': '/usr/share'};
    final dirs = xdgUserDirs(env);
    expect(dirs, {
      'DESKTOP': '${home.path}/Scrivania',
      'DOCUMENTS': '${home.path}/Documenti',
      'DOWNLOAD': '/srv/scaricati',
    });
    expect(specialFolderKey('${home.path}/Documenti/', env: env), 'DOCUMENTS');
    expect(specialFolderKey('/srv/scaricati', env: env), 'DOWNLOAD');
    expect(specialFolderKey(home.path, env: env), 'HOME');
    expect(
      specialFolderKey('/usr/share/applications', env: env),
      'APPLICATIONS',
    );
    expect(
      specialFolderKey('${home.path}/Applications', env: env),
      'APPLICATIONS',
    );
    expect(specialFolderKey('${home.path}/.local/share', env: env), 'LIBRARY');
    expect(specialFolderKey('${home.path}/Progetti', env: env), isNull);
    expect(specialFolderKey('', env: env), isNull);
    expect(xdgUserDirs(const {}), isEmpty);
  });

  test(
    'l\'icona di una cartella: simbolo, tinta, ripiego sulla cartella semplice',
    () {
      final root = folderTree();
      addTearDown(() => root.deleteSync(recursive: true));
      final env = {'SILICA_ICONS_DIR': root.path, 'HOME': '/home/prova'};
      final home = resolveFolderIcon('/home/prova', env: env)!;
      expect(home.icon.id, 'folder-generic');
      expect(home.symbol, 'house');
      expect(home.tint, isNull);
      // Una cartella qualsiasi senza tinta: la cartella semplice.
      expect(resolveFolderIcon('/tmp/x', env: env)!.icon.id, 'folder');
      // Simbolo mappato ma assente da symbols/: ripiego.
      final missing = resolveFolderIcon('/tmp/x', key: 'DOWNLOAD', env: env)!;
      expect(missing.icon.id, 'folder');
      expect(missing.symbol, isNull);
      // Tinta senza simbolo: la cartella generica tinta.
      final green = resolveFolderIcon('/tmp/x', tint: 'green', env: env)!;
      expect(green.icon.id, 'folder-generic');
      expect(green.symbol, isNull);
      expect(green.tint, const Color(0xFF2FB350));
      // Un simbolo scelto da chi chiama vince sulla mappa.
      expect(
        resolveFolderIcon('/home/prova', symbol: 'doc-text', env: env)!.symbol,
        'doc-text',
      );
      expect(
        resolveFolderIcon('/x', symbol: '../../etc/passwd', env: env)!.icon.id,
        'folder',
      );
      // Senza nessuna icona di cartella: null.
      expect(
        resolveFolderIcon('/x', env: {'SILICA_ICONS_DIR': '/nessuna'}),
        isNull,
      );
    },
  );

  testWidgets(
    'la cartella generica si compone con simbolo e tinta a ogni misura',
    (tester) async {
      final root = folderTree();
      addTearDown(() => root.deleteSync(recursive: true));
      final env = {'SILICA_ICONS_DIR': root.path, 'HOME': '/home/prova'};
      final home = resolveFolderIcon('/home/prova', tint: '#e8336d', env: env)!;
      await tester.pumpWidget(
        Directionality(
          textDirection: TextDirection.ltr,
          child: Wrap(
            children: [
              for (final side in [12.0, 32.0, 64.0, 128.0, 256.0])
                for (final dark in [false, true])
                  ShellIcon(FolderSource(home), side: side, dark: dark),
              ShellIcon(
                FolderSource(resolveFolderIcon('/tmp', env: env)!),
                side: 48,
              ),
            ],
          ),
        ),
      );
      expect(tester.takeException(), isNull);
      expect(find.byType(ColorFiltered), findsNWidgets(10));
      // 12 pt: niente simbolo; 32 pt: solo il simbolo; da 64 riflesso e simbolo in chiaro,
      // solo il simbolo in scuro (riflesso a opacità 0).
      expect(find.byType(Opacity), findsNWidgets(0 + 2 + 3 + 3 + 3));
    },
  );

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
