import 'dart:ffi';
import 'dart:io';
import 'dart:typed_data';
import 'dart:ui' show Color;

import 'package:ffi/ffi.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:silica_shell/dock/model.dart';
import 'package:silica_shell/system/folder_icons.dart';
import 'package:silica_shell/system/folder_style.dart';
import 'package:silica_ui/silica_ui.dart';

const String _svg =
    '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1024 1024">'
    '<rect x="100" y="100" width="824" height="824" fill="#2f8cff"/></svg>';

const String _generic = '''
{
  "format": 1, "id": "folder-generic", "name": "Folder Generic", "kind": "file",
  "layers": [ { "role": "artwork", "file": "icon.svg" } ],
  "variants": { "full": { "artwork": "icon-full.svg" } },
  "dynamic": {
    "type": "folder-symbol",
    "symbol": {
      "box": [353.9, 424.36, 316.19, 316.19],
      "sizes": [ { "min_size": 16, "weight": "regular", "scale": 1 } ]
    },
    "effect": { "light": { "color": "#1a64c8", "opacity": 0.78 } },
    "tint": {
      "light": { "luminance": [0.35, 0.85], "shade": 0.72, "glow": 0.5 },
      "labels": { "red": "#f2382e", "blue": "#2f7cf6", "green": "#2fb350" }
    },
    "folders": { "MUSIC": "music-note" },
    "fallback": "folder"
  }
}
''';

const String _plain = '''
{
  "format": 1, "id": "folder", "name": "Folder", "kind": "file",
  "layers": [ { "role": "artwork", "file": "icon.svg" } ],
  "variants": { "full": { "artwork": "icon-full.svg" } },
  "dynamic": null
}
''';

Directory iconsRoot() {
  final root = Directory.systemTemp.createTempSync('silica-icons-');
  for (final (id, manifest) in [
    ('folder-generic', _generic),
    ('folder', _plain),
  ]) {
    final dir = Directory('${root.path}/files/$id')
      ..createSync(recursive: true);
    File('${dir.path}/icon.json').writeAsStringSync(manifest);
    for (final name in ['icon', 'icon-full']) {
      File('${dir.path}/$name.svg').writeAsStringSync(_svg);
    }
  }
  final symbols = Directory('${root.path}/symbols/regular')
    ..createSync(recursive: true);
  for (final name in ['music-note', 'star']) {
    File('${symbols.path}/$name.svg').writeAsStringSync(
      '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24">'
      '<circle cx="12" cy="12" r="8"/></svg>',
    );
  }
  return root;
}

/// Attributes in memory; `supported: false` acts like a disk without user attributes.
FolderStyles fakeStyles(
  Map<String, Map<String, String>> attributes, {
  bool supported = true,
  Map<String, String> gio = const {},
}) => FolderStyles(
  readAttribute: (path, name) => (
    value: supported ? (attributes[path]?[name]) : null,
    unsupported: !supported,
  ),
  readMetadata: (path) => gio[path],
);

typedef _SetC = Int32 Function(
  Pointer<Utf8>,
  Pointer<Utf8>,
  Pointer<Uint8>,
  IntPtr,
  Int32,
);
typedef _SetDart = int Function(
  Pointer<Utf8>,
  Pointer<Utf8>,
  Pointer<Uint8>,
  int,
  int,
);

/// Writes an attribute as the file manager would.
bool setAttribute(String path, String name, String value) {
  final set = DynamicLibrary.process().lookupFunction<_SetC, _SetDart>(
    'setxattr',
  );
  final bytes = Uint8List.fromList(value.codeUnits);
  final p = path.toNativeUtf8();
  final n = name.toNativeUtf8();
  final buffer = calloc<Uint8>(bytes.length);
  buffer.asTypedList(bytes.length).setAll(0, bytes);
  final r = set(p, n, buffer, bytes.length, 0);
  calloc
    ..free(p)
    ..free(n)
    ..free(buffer);
  return r == 0;
}

void main() {
  test('the stored JSON: valid fields only', () {
    expect(
      FolderStyle.parse('{"tint":"#E8336D","symbol":"music-note"}'),
      const FolderStyle(tint: '#e8336d', symbol: 'music-note'),
    );
    expect(FolderStyle.parse('{"tint":"red"}'), const FolderStyle(tint: 'red'));
    expect(FolderStyle.parse('{"tint":"pink","symbol":"../x"}'), isNull);
    expect(FolderStyle.parse('nope'), isNull);
    expect(FolderStyle.parse(null), isNull);
  });

  test('tint order: custom, then the first colour tag, then none', () {
    final attributes = <String, Map<String, String>>{
      '/a': {},
      '/b': {folderTagsAttribute: 'Lavoro,Verde,Red'},
      '/c': {
        folderTagsAttribute: 'Red',
        folderStyleAttribute: '{"tint":"#123456","symbol":"star"}',
      },
      '/d': {
        folderTagsAttribute: 'Blu',
        folderStyleAttribute: '{"symbol":"star"}',
      },
    };
    final styles = fakeStyles(attributes);
    expect(styles.look('/a'), (symbol: null, tint: null));
    expect(styles.look('/b'), (symbol: null, tint: 'green'));
    expect(styles.look('/c'), (symbol: 'star', tint: '#123456'));
    expect(styles.look('/d'), (symbol: 'star', tint: 'blue'));
    // Cached until a watch reports a change.
    attributes['/a']![folderTagsAttribute] = 'Rosso';
    expect(styles.look('/a').tint, isNull);
    styles.invalidate(['/a']);
    expect(styles.look('/a').tint, 'red');
  });

  test('a disk without attributes is read from the GIO store', () {
    final styles = fakeStyles(
      {},
      supported: false,
      gio: {'/media/usb/Foto': '{"symbol":"star","tint":"blue"}'},
    );
    expect(styles.look('/media/usb/Foto'), (symbol: 'star', tint: 'blue'));
    expect(styles.look('/media/usb/Altro'), (symbol: null, tint: null));
    expect(
      parseGioInfo('attributes:\n  metadata::silica-folder: {"tint":"red"}\n'),
      '{"tint":"red"}',
    );
    // Tests start nothing: the real store answers nothing.
    expect(readGioFolderStyle('/'), isNull);
  });

  test('real attributes: missing, set, and an unsupported file system', () {
    final cache = Directory('${Platform.environment['HOME']}/.cache')
      ..createSync(recursive: true);
    final dir = cache.createTempSync('shell-folder-style');
    addTearDown(() => dir.deleteSync(recursive: true));
    final read = readFolderAttribute(dir.path, folderStyleAttribute);
    if (read.unsupported ||
        !setAttribute(dir.path, folderStyleAttribute, '{"tint":"red"}')) {
      markTestSkipped('the file system does not support user attributes');
      return;
    }
    expect(read.value, isNull);
    expect(setAttribute(dir.path, folderTagsAttribute, 'Green'), isTrue);
    final styles = FolderStyles(readMetadata: (_) => fail('not asked'));
    expect(styles.look(dir.path), (symbol: null, tint: 'red'));
    expect(readFolderAttribute('${dir.path}/gone', 'user.x').value, isNull);
    if (Directory('/proc/self').existsSync()) {
      expect(
        readFolderAttribute('/proc/self', folderStyleAttribute).unsupported,
        isTrue,
      );
    }
  });

  test('the icon gets symbol and tint, empty or full', () {
    final root = iconsRoot();
    addTearDown(() => root.deleteSync(recursive: true));
    final home = Directory.systemTemp.createTempSync('shell-folder-home');
    addTearDown(() => home.deleteSync(recursive: true));
    final env = {
      'SILICA_ICONS_DIR': root.path,
      'HOME': home.path,
      'XDG_CONFIG_HOME': '${home.path}/.config',
    };
    File('${home.path}/.config/user-dirs.dirs')
      ..createSync(recursive: true)
      ..writeAsStringSync('XDG_MUSIC_DIR="\$HOME/Music"\n');
    final music = '${home.path}/Music';
    final plain = '${home.path}/Plain';
    final styles = fakeStyles({
      plain: {
        folderTagsAttribute: 'Blue',
        folderStyleAttribute: '{"symbol":"star"}',
      },
      music: {folderStyleAttribute: '{"symbol":"no-such-symbol","tint":"red"}'},
    });
    expect(
      styledFolderIcon('${home.path}/Other', env: env, styles: styles)!.icon.id,
      'folder',
    );
    final folder = styledFolderIcon(plain, env: env, styles: styles)!;
    expect(folder.symbol, 'star');
    expect(folder.tint, const Color(0xFF2F7CF6));
    final full = fullFolder(folder);
    expect((full.symbol, full.tint), (folder.symbol, folder.tint));
    expect(full.icon.layers.single.path, endsWith('/icon-full.svg'));
    // A symbol silica-icons lacks keeps the special folder's own.
    final special = styledFolderIcon(music, env: env, styles: styles)!;
    expect(special.symbol, 'music-note');
    expect(special.tint, const Color(0xFFF2382E));
  });

  test('a Dock folder with a colour or symbol is drawn as a folder icon', () {
    final root = iconsRoot();
    addTearDown(() => root.deleteSync(recursive: true));
    final env = {'SILICA_ICONS_DIR': root.path};
    final generic = findSilicaIcon('folder-generic', env: env)!;
    final plain = findSilicaIcon('folder', env: env)!;
    final styled = FolderIcon(generic, symbol: 'star');
    final item = Item.app(
      'Progetti',
      folder: '/home/a/Progetti',
      layered: plain,
      folderIcon: styled,
    );
    expect(item.icon, isA<FolderSource>());
    expect((item.icon! as FolderSource).folder.symbol, 'star');
    final without = Item.app('Download', folder: '/x', layered: plain);
    expect(without.icon, isA<LayeredSource>());
  });
}
