// The Dock items: the pinned apps (~/.config/silica/dock.json), then the open ones that are
// not pinned, the separator, the Download folder and the trash. The open windows come from
// Ossido (ShellChannel).

import 'dart:convert';
import 'dart:io';
import 'dart:ui' as ui;
import 'package:silica_ui/silica_ui.dart';

import '../system/desktop_entries.dart';
import '../system/icon_lookup.dart';
import '../system/shell_channel.dart';
import '../system/silica_icons.dart';
import 'model.dart';

File _dockConfig(Map<String, String>? env) {
  final e = env ?? Platform.environment;
  final config = e['XDG_CONFIG_HOME'] ?? '${e['HOME'] ?? ''}/.config';
  return File('$config/silica/dock.json');
}

/// The Dock configuration ($XDG_CONFIG_HOME/silica/dock.json, usually ~/.config):
/// `pinned`, `magnify`, `recents`.
Map<String, Object?> readDockConfig({Map<String, String>? env}) {
  final file = _dockConfig(env);
  try {
    if (!file.existsSync()) return {};
    final json = jsonDecode(file.readAsStringSync());
    return json is Map
        ? {for (final e in json.entries) '${e.key}': e.value}
        : {};
  } on FileSystemException {
    return {};
  } on FormatException {
    // Broken configuration: the defaults.
    return {};
  }
}

/// Changes one key of the configuration leaving the others.
void saveDockOption(String key, Object? value, {Map<String, String>? env}) {
  final file = _dockConfig(env);
  final config = readDockConfig(env: env)..[key] = value;
  try {
    file.parent.createSync(recursive: true);
    file.writeAsStringSync(jsonEncode(config));
  } on FileSystemException {
    // Without a writable configuration the choice lasts until the end of the session.
  }
}

/// A yes/no Dock option.
bool dockFlag(String key, {required bool fallback, Map<String, String>? env}) {
  final value = readDockConfig(env: env)[key];
  return value is bool ? value : fallback;
}

/// Saves the pinned apps ("Keep in Dock" and "Remove").
void savePinned(List<String> ids, {Map<String, String>? env}) =>
    saveDockOption('pinned', ids, env: env);

/// Apps pinned when the configuration is missing: the first installed ones of this list.
const List<List<String>> defaultPinned = [
  ['dev.frascella.Mustache', 'org.gnome.Nautilus'],
  [
    'firefox_firefox',
    'firefox',
    'org.mozilla.firefox',
    'chromium_chromium',
    'google-chrome',
  ],
  ['dev.frascella.Mail'],
  [
    'dev.frascella.Terminal',
    'org.gnome.Ptyxis',
    'org.gnome.Terminal',
    'org.gnome.Console',
  ],
  ['org.gnome.TextEditor', 'org.gnome.gedit'],
  ['code', 'code_code'],
  ['dev.frascella.Calculator', 'org.gnome.Calculator'],
  ['dev.frascella.Clock', 'org.gnome.clocks'],
  ['org.gnome.Calendar'],
  ['org.gnome.Settings', 'gnome-control-center'],
];

/// How many recently closed apps the recents section holds (D-08: 3 observed).
const int recentAppsMax = 3;

/// The pinned apps: from the configuration, or from the default list.
List<String> pinnedIds(
  Map<String, DesktopEntry> entries, {
  Map<String, String>? env,
}) {
  final pinned = readDockConfig(env: env)['pinned'];
  if (pinned is List) return [for (final id in pinned) '$id'];
  return [
    for (final choices in defaultPinned)
      if (choices.where(entries.containsKey).firstOrNull case final id?) id,
  ];
}

Item _appItem(
  DesktopEntry entry,
  List<Toplevel> windows, {
  bool pinned = false,
}) => Item.app(
  entry.name,
  running: windows.isNotEmpty,
  iconPath: findIcon(entry.icon),
  layered: silicaIconFor(entry),
  command: entry.exec,
  openFiles: entry.rawExec,
  windows: [for (final w in windows) w.id],
  id: entry.id,
  pinned: pinned,
);

/// The Dock items for these apps and these windows.
List<Item> dockItems(
  Map<String, DesktopEntry> entries,
  List<String> pinned,
  List<Toplevel> toplevels, {
  List<String> minimized = const [],
  Map<String, ui.Image> thumbnails = const {},
  List<String>? recents,
  String trashLabel = 'Trash',
}) {
  List<Toplevel> windowsOf(DesktopEntry entry) => [
    for (final t in toplevels)
      if (entry.matchesAppId(t.appId)) t,
  ];
  final items = <Item>[];
  final shown = <String>{};
  for (final id in pinned) {
    final entry = entries[id];
    if (entry == null) continue;
    items.add(_appItem(entry, windowsOf(entry), pinned: true));
    shown.add(id);
  }
  // With recent apps (D-08) a second separator after the pinned ones: between the two the
  // open apps that are not pinned, then the last closed ones until the section has 3.
  final recentSection = recents != null;
  final before = items.length;
  // Open apps that are not pinned, in the order they appeared.
  for (final window in toplevels) {
    final entry = entries.values
        .where((e) => e.matchesAppId(window.appId))
        .firstOrNull;
    if (entry == null || !shown.add(entry.id)) continue;
    items.add(_appItem(entry, windowsOf(entry)));
  }
  if (recentSection) {
    for (final id in recents) {
      if (items.length - before >= recentAppsMax) break;
      final entry = entries[id];
      if (entry == null || !shown.add(id)) continue;
      items.add(_appItem(entry, const []));
    }
    if (items.length > before) items.insert(before, const Item.separator());
  }
  final home = Platform.environment['HOME'] ?? '';
  final trashFull = trashHasFiles(home);
  return [
    ...items,
    const Item.separator(),
    Item.app(
      // The folder's own name, as on disk.
      downloadDir(home).split('/').where((p) => p.isNotEmpty).lastOrNull ??
          'Downloads',
      folder: downloadDir(home),
      iconPath: findIcon('folder-download'),
      layered: findSilicaIcon('folder'),
      command: 'xdg-open "$home/Download" || xdg-open "$home/Downloads"',
    ),
    // The minimized windows, from the first minimized, before the Trash (D-08).
    for (final id in minimized)
      if (toplevels.where((t) => t.id == id).firstOrNull case final window?)
        _windowItem(window, entries, thumbnails[id]),
    Item.app(
      trashLabel,
      iconPath: findIcon(trashFull ? 'user-trash-full' : 'user-trash'),
      layered: findSilicaIcon(trashFull ? 'trash-full' : 'trash-empty'),
      command: 'xdg-open trash:///',
      trash: true,
    ),
  ];
}

Item _windowItem(
  Toplevel window,
  Map<String, DesktopEntry> entries,
  ui.Image? thumbnail,
) {
  final entry = entries.values
      .where((e) => e.matchesAppId(window.appId))
      .firstOrNull;
  return Item.window(
    window.title.isNotEmpty ? window.title : (entry?.name ?? window.appId),
    window: window.id,
    thumbnail: thumbnail,
    iconPath: entry == null ? null : findIcon(entry.icon),
    layered: entry == null ? null : silicaIconFor(entry),
  );
}

/// The downloads folder: the xdg-user-dirs one, otherwise ~/Download or ~/Downloads.
String downloadDir(String home, {Map<String, String>? env}) {
  try {
    // xdg-user-dirs: in $XDG_CONFIG_HOME, if there.
    final config =
        (env ?? Platform.environment)['XDG_CONFIG_HOME'] ?? '$home/.config';
    final dirs = File('$config/user-dirs.dirs');
    if (dirs.existsSync()) {
      for (final line in dirs.readAsLinesSync()) {
        if (line.startsWith('XDG_DOWNLOAD_DIR=')) {
          final value = line
              .substring('XDG_DOWNLOAD_DIR='.length)
              .replaceAll('"', '')
              .replaceAll(r'$HOME', home);
          if (Directory(value).existsSync()) return value;
        }
      }
    }
  } on FileSystemException {
    // Try the usual names.
  }
  for (final name in ['Download', 'Downloads', 'Scaricati']) {
    if (Directory('$home/$name').existsSync()) return '$home/$name';
  }
  return '$home/Download';
}

/// The user's trash contains something (freedesktop Trash specification).
bool trashHasFiles(String home) {
  try {
    final files = Directory('$home/.local/share/Trash/files');
    return files.existsSync() && files.listSync().isNotEmpty;
  } on FileSystemException {
    return false;
  }
}
