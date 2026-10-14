// Screenshot files: the user folders they go to, their names, moving them, and the app that
// opens images (the default handler of image/png, from the freedesktop mimeapps.list files).

import 'dart:io';

import '../l10n/l10n.dart';

/// A folder from xdg-user-dirs (`XDG_DESKTOP_DIR`, `XDG_DOCUMENTS_DIR`), or the first of
/// `names` that exists in the home, or the home.
String userFolder(String key, List<String> names, {Map<String, String>? env}) {
  final e = env ?? Platform.environment;
  final home = e['HOME'] ?? '/tmp';
  final config = e['XDG_CONFIG_HOME'] ?? '$home/.config';
  try {
    final dirs = File('$config/user-dirs.dirs');
    if (dirs.existsSync()) {
      for (final line in dirs.readAsLinesSync()) {
        if (line.startsWith('$key=')) {
          final value = line
              .substring(key.length + 1)
              .replaceAll('"', '')
              .replaceAll(r'$HOME', home);
          if (Directory(value).existsSync()) return value;
        }
      }
    }
  } on FileSystemException {
    // Try the usual names.
  }
  for (final name in names) {
    if (Directory('$home/$name').existsSync()) return '$home/$name';
  }
  return home;
}

/// Where screenshots go by default: the Desktop.
String screenshotFolder({Map<String, String>? env}) =>
    userFolder('XDG_DESKTOP_DIR', const ['Scrivania', 'Desktop'], env: env);

/// The Documents folder.
String documentsFolder({Map<String, String>? env}) =>
    userFolder('XDG_DOCUMENTS_DIR', const ['Documenti', 'Documents'], env: env);

/// Where captures meant for the clipboard or an app are written when they need a file (a
/// drag, an app opening them): $XDG_CACHE_HOME/silica/screenshots.
String screenshotCache({Map<String, String>? env}) {
  final e = env ?? Platform.environment;
  final home = e['HOME'] ?? '/tmp';
  return '${e['XDG_CACHE_HOME'] ?? '$home/.cache'}/silica/screenshots';
}

/// The file name: "Screenshot 2026-10-14 at 04.12.33.png" in the user's language, from the
/// capture time (a number if it already exists).
String screenshotName(
  DateTime t,
  bool Function(String name) exists, {
  AppLocalizations? texts,
}) {
  String two(int v) => v.toString().padLeft(2, '0');
  final base = (texts ?? englishTexts).screenshotFileName(
    '${t.year}-${two(t.month)}-${two(t.day)}',
    '${two(t.hour)}.${two(t.minute)}.${two(t.second)}',
  );
  var name = '$base.png';
  for (var i = 2; exists(name); i++) {
    name = '$base ($i).png';
  }
  return name;
}

/// Moves `source` to `target` (copy and delete across disks); false if it failed.
bool moveFile(String source, String target) {
  try {
    final from = File(source);
    try {
      from.renameSync(target);
    } on FileSystemException {
      from.copySync(target);
      from.deleteSync();
    }
    return true;
  } on FileSystemException {
    return false;
  }
}

/// The desktop entry id (without `.desktop`) of the default app for `mime`: the first
/// `mimeapps.list` that names one, in the specification's order ($XDG_CONFIG_HOME, then
/// $XDG_CONFIG_DIRS, then the `applications/` of $XDG_DATA_HOME and $XDG_DATA_DIRS).
/// `exists` tells whether an entry id is installed.
String? defaultAppFor(
  String mime, {
  Map<String, String>? env,
  required bool Function(String id) exists,
}) {
  final e = env ?? Platform.environment;
  final home = e['HOME'] ?? '/tmp';
  final configHome = e['XDG_CONFIG_HOME'] ?? '$home/.config';
  final configDirs = (e['XDG_CONFIG_DIRS'] ?? '/etc/xdg')
      .split(':')
      .where((d) => d.isNotEmpty);
  final dataHome = e['XDG_DATA_HOME'] ?? '$home/.local/share';
  final dataDirs = (e['XDG_DATA_DIRS'] ?? '/usr/local/share:/usr/share')
      .split(':')
      .where((d) => d.isNotEmpty);
  final desktop = (e['XDG_CURRENT_DESKTOP'] ?? '')
      .toLowerCase()
      .split(':')
      .where((d) => d.isNotEmpty);
  final lists = <String>[
    for (final d in [configHome, ...configDirs]) ...[
      for (final name in desktop) '$d/$name-mimeapps.list',
      '$d/mimeapps.list',
    ],
    for (final d in [dataHome, ...dataDirs]) ...[
      for (final name in desktop) '$d/applications/$name-mimeapps.list',
      '$d/applications/mimeapps.list',
    ],
  ];
  for (final path in lists) {
    final String text;
    try {
      text = File(path).readAsStringSync();
    } on FileSystemException {
      continue;
    }
    final found = defaultInMimeapps(text, mime, exists);
    if (found != null) return found;
  }
  return null;
}

/// The first installed app the `[Default Applications]` group of a mimeapps.list gives for
/// `mime`.
String? defaultInMimeapps(
  String text,
  String mime,
  bool Function(String id) exists,
) {
  var inDefaults = false;
  for (final raw in text.split('\n')) {
    final line = raw.trim();
    if (line.startsWith('[')) {
      inDefaults = line == '[Default Applications]';
      continue;
    }
    if (!inDefaults || !line.startsWith('$mime=')) continue;
    for (final id in line.substring(mime.length + 1).split(';')) {
      final name = id.trim();
      if (!name.endsWith('.desktop')) continue;
      final bare = name.substring(0, name.length - '.desktop'.length);
      if (bare.isNotEmpty && exists(bare)) return bare;
    }
  }
  return null;
}
