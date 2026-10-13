// Gli elementi del Dock: le app fissate (~/.config/silica/dock.json), poi quelle aperte
// e non fissate, il separatore, la cartella Download e il cestino. Le finestre aperte
// arrivano da Ossido (ShellChannel).

import 'dart:convert';
import 'dart:io';

import '../system/desktop_entries.dart';
import '../system/icon_lookup.dart';
import '../system/shell_channel.dart';
import 'model.dart';

/// App fissate quando manca la configurazione: le prime installate di questo elenco.
const List<List<String>> defaultPinned = [
  ['org.gnome.Nautilus'],
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

Item _appItem(DesktopEntry entry, List<Toplevel> windows) => Item.app(
  entry.name,
  running: windows.isNotEmpty,
  iconPath: findIcon(entry.icon),
  command: entry.exec,
  windows: [for (final w in windows) w.id],
);

/// Gli elementi del Dock per queste app e queste finestre.
List<Item> dockItems(
  Map<String, DesktopEntry> entries,
  List<String> pinned,
  List<Toplevel> toplevels,
) {
  List<Toplevel> windowsOf(DesktopEntry entry) => [
    for (final t in toplevels)
      if (entry.matchesAppId(t.appId)) t,
  ];
  final items = <Item>[];
  final shown = <String>{};
  for (final id in pinned) {
    final entry = entries[id];
    if (entry == null) continue;
    items.add(_appItem(entry, windowsOf(entry)));
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
  return [
    ...items,
    const Item.separator(),
    Item.app(
      'Download',
      iconPath: findIcon('folder-download'),
      command: 'xdg-open "$home/Download" || xdg-open "$home/Downloads"',
    ),
    Item.app(
      'Cestino',
      iconPath: findIcon('user-trash'),
      command: 'xdg-open trash:///',
    ),
  ];
}
