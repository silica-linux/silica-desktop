// Gli elementi del Dock: le app fissate (~/.config/silica/dock.json), poi quelle aperte
// e non fissate, il separatore, la cartella Download e il cestino. Le finestre aperte
// arrivano da Ossido (ShellChannel).

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

/// La configurazione del Dock ($XDG_CONFIG_HOME/silica/dock.json, di solito ~/.config):
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
    // Configurazione rovinata: le predefinite.
    return {};
  }
}

/// Cambia una chiave della configurazione lasciando le altre.
void saveDockOption(String key, Object? value, {Map<String, String>? env}) {
  final file = _dockConfig(env);
  final config = readDockConfig(env: env)..[key] = value;
  try {
    file.parent.createSync(recursive: true);
    file.writeAsStringSync(jsonEncode(config));
  } on FileSystemException {
    // Senza configurazione scrivibile la scelta vale fino alla fine della sessione.
  }
}

/// Un'opzione sì/no del Dock.
bool dockFlag(String key, {required bool fallback, Map<String, String>? env}) {
  final value = readDockConfig(env: env)[key];
  return value is bool ? value : fallback;
}

/// Salva le app fissate ("Mantieni nel Dock" e "Rimuovi").
void savePinned(List<String> ids, {Map<String, String>? env}) =>
    saveDockOption('pinned', ids, env: env);

/// App fissate quando manca la configurazione: le prime installate di questo elenco.
const List<List<String>> defaultPinned = [
  ['dev.frascella.Mustache', 'org.gnome.Nautilus'],
  [
    'firefox_firefox',
    'firefox',
    'org.mozilla.firefox',
    'chromium_chromium',
    'google-chrome',
  ],
  ['org.gnome.Ptyxis', 'org.gnome.Terminal', 'org.gnome.Console'],
  ['org.gnome.TextEditor', 'org.gnome.gedit'],
  ['code', 'code_code'],
  ['org.gnome.Calculator'],
  ['org.gnome.Calendar'],
  ['org.gnome.Settings', 'gnome-control-center'],
];

/// Quante app chiuse di recente stanno nella sezione delle recenti (D-08: 3 osservate).
const int recentAppsMax = 3;

/// Le app fissate: dalla configurazione, o dall'elenco predefinito.
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

/// Gli elementi del Dock per queste app e queste finestre.
List<Item> dockItems(
  Map<String, DesktopEntry> entries,
  List<String> pinned,
  List<Toplevel> toplevels, {
  List<String> minimized = const [],
  Map<String, ui.Image> thumbnails = const {},
  List<String>? recents,
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
  // Con le app recenti (D-08) un secondo separatore dopo le fissate: fra i due le app aperte
  // non fissate, poi le ultime chiuse finché la sezione ne ha 3.
  final recentSection = recents != null;
  final before = items.length;
  // App aperte e non fissate, nell'ordine in cui sono comparse.
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
      'Download',
      folder: downloadDir(home),
      iconPath: findIcon('folder-download'),
      layered: findSilicaIcon('folder'),
      command: 'xdg-open "$home/Download" || xdg-open "$home/Downloads"',
    ),
    // Le finestre ridotte, dalla prima ridotta, prima del Cestino (D-08).
    for (final id in minimized)
      if (toplevels.where((t) => t.id == id).firstOrNull case final window?)
        _windowItem(window, entries, thumbnails[id]),
    Item.app(
      'Cestino',
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

/// La cartella dei download: quella di xdg-user-dirs, altrimenti ~/Download o ~/Downloads.
String downloadDir(String home, {Map<String, String>? env}) {
  try {
    // xdg-user-dirs: in $XDG_CONFIG_HOME, se c'è.
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
    // Si prova con i nomi soliti.
  }
  for (final name in ['Download', 'Downloads', 'Scaricati']) {
    if (Directory('$home/$name').existsSync()) return '$home/$name';
  }
  return '$home/Download';
}

/// Il cestino dell'utente contiene qualcosa (specifica freedesktop Trash).
bool trashHasFiles(String home) {
  try {
    final files = Directory('$home/.local/share/Trash/files');
    return files.existsSync() && files.listSync().isNotEmpty;
  } on FileSystemException {
    return false;
  }
}
