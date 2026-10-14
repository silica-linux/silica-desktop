import 'dart:io';
import 'dart:ui' show Color;

import 'package:flutter_test/flutter_test.dart';
import 'package:silica_shell/system/folder_icons.dart';
import 'package:silica_ui/silica_ui.dart';

const String svg =
    '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1024 1024">'
    '<rect x="100" y="100" width="824" height="824" fill="#2f8cff"/></svg>';

const String folderManifest = '''
{
  "format": 1, "id": "folder", "name": "Folder", "kind": "file",
  "layers": [ { "role": "artwork", "file": "icon.svg" } ],
  "variants": {
    "dark": { "artwork": "icon-dark.svg" },
    "full": { "artwork": "icon-full.svg" },
    "full-dark": { "artwork": "icon-full-dark.svg" }
  },
  "dynamic": null
}
''';

Directory iconRoot({bool full = true}) {
  final root = Directory.systemTemp.createTempSync('silica-icons-');
  final dir = Directory('${root.path}/files/folder')
    ..createSync(recursive: true);
  File('${dir.path}/icon.json').writeAsStringSync(
    full
        ? folderManifest
        : folderManifest.replaceAll(RegExp(r'"full[^}]*\}'), '"x": {}'),
  );
  for (final name in ['icon', 'icon-dark', 'icon-full', 'icon-full-dark']) {
    File('${dir.path}/$name.svg').writeAsStringSync(svg);
  }
  return root;
}

void main() {
  test('a folder is full with one visible item; hidden files do not count', () {
    final dir = Directory.systemTemp.createTempSync('full-folder');
    addTearDown(() => dir.deleteSync(recursive: true));
    expect(folderHasItems(dir.path), isFalse);
    File('${dir.path}/.hidden').writeAsStringSync('');
    Directory('${dir.path}/.cache').createSync();
    expect(folderHasItems(dir.path), isFalse);
    File('${dir.path}/a.txt').writeAsStringSync('');
    expect(folderHasItems(dir.path), isTrue);
    // Gone or unreadable: empty, not an exception.
    expect(folderHasItems('${dir.path}/missing'), isFalse);
  });

  test('the answer follows the folder when its contents change', () {
    final dir = Directory.systemTemp.createTempSync('full-folder');
    addTearDown(() => dir.deleteSync(recursive: true));
    final file = File('${dir.path}/a.txt')..writeAsStringSync('');
    expect(folderHasItems(dir.path), isTrue);
    file.deleteSync();
    // Make the modification time differ even on a coarse clock.
    final later = DateTime.now().millisecondsSinceEpoch ~/ 1000 + 5;
    Process.runSync('touch', ['-d', '@$later', dir.path]);
    expect(folderHasItems(dir.path), isFalse);
  });

  test('the full variant swaps the artwork and keeps id and dark', () {
    final root = iconRoot();
    addTearDown(() => root.deleteSync(recursive: true));
    final env = {'SILICA_ICONS_DIR': root.path};
    final plain = findSilicaIcon('folder', env: env)!;
    final full = fullFolderVariant(plain);
    expect(full.id, 'folder');
    expect(full.layers.single.path, endsWith('/icon-full.svg'));
    expect(full.layers.single.darkPath, endsWith('/icon-full-dark.svg'));
    expect(plain.layers.single.path, endsWith('/icon.svg'));
  });

  test('an icon without a full variant stays as it is', () {
    final root = iconRoot(full: false);
    addTearDown(() => root.deleteSync(recursive: true));
    final plain = findSilicaIcon(
      'folder',
      env: {'SILICA_ICONS_DIR': root.path},
    );
    expect(plain, isNotNull);
    expect(identical(fullFolderVariant(plain!), plain), isTrue);
  });

  test('fullFolder keeps symbol and tint', () {
    final root = iconRoot();
    addTearDown(() => root.deleteSync(recursive: true));
    final plain = findSilicaIcon(
      'folder',
      env: {'SILICA_ICONS_DIR': root.path},
    )!;
    const tint = Color(0xFF2FB350);
    final full = fullFolder(FolderIcon(plain, symbol: 'house', tint: tint));
    expect(full.symbol, 'house');
    expect(full.tint, tint);
    expect(full.icon.layers.single.path, endsWith('/icon-full.svg'));
  });
}
