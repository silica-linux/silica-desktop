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

/// Salva le app fissate in ~/.config/silica/dock.json ("Mantieni nel Dock" e "Rimuovi").
void savePinned(List<String> ids, {Map<String, String>? env}) {
  final home = (env ?? Platform.environment)['HOME'] ?? '';
  final config = File('$home/.config/silica/dock.json');
  try {
    config.parent.createSync(recursive: true);
    config.writeAsStringSync(jsonEncode({'pinned': ids}));
  } on FileSystemException {
    // Senza configurazione scrivibile la scelta vale fino alla fine della sessione.
  }
}

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

/// Le app fissate: dalla configurazione, o dall'elenco predefinito.
List<String> pinnedIds(
  Map<String, DesktopEntry> entries, {
  Map<String, String>? env,
}) {
  final home = (env ?? Platform.environment)['HOME'] ?? '';
  final config = File('$home/.config/silica/dock.json');
  if (config.existsSync()) {
    try {
      final json = jsonDecode(config.readAsStringSync());
      final pinned = json is Map ? json['pinned'] : null;
      if (pinned is List) return [for (final id in pinned) '$id'];
    } on FormatException {
      // Configurazione rovinata: si usa il predefinito.
    }
  }
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
  // App aperte e non fissate, nell'ordine in cui sono comparse.
  for (final window in toplevels) {
    final entry = entries.values
        .where((e) => e.matchesAppId(window.appId))
        .firstOrNull;
    if (entry == null || !shown.add(entry.id)) continue;
    items.add(_appItem(entry, windowsOf(entry)));
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
