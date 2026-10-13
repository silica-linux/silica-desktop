import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:silica_shell/system/desktop_entries.dart';
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
}
