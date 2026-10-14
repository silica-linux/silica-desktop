// The Control Center's modules: which ones, in which order and at which size, packed into
// the measured grid (docs/spec/controlcenter.md, CC-00, SH-CC-01, INV-A2).
//
// The grid is four 64 pt cells across (292 pt: 4 × 64 + 3 × 12), with a 76 pt pitch in
// both directions. A small module is one cell (a 64 pt round tile), a medium one two cells
// across (a 140 × 64 capsule), a large one two by two (140 × 140), a wide one the whole row
// (292 × 64, Display and Sound). Modules are placed in order, each at the first free spot
// reading row by row, which reproduces the reference's default layout.
//
// The user's arrangement is kept in $XDG_CONFIG_HOME/silica/controlcenter.json:
// {"modules": [{"id": "wifi", "size": "medium"}, ...]}. Edit mode (Edit Controls) and the
// module menu change it.

import 'dart:convert';
import 'dart:io';
import 'dart:ui';

import 'package:flutter/foundation.dart';

import '../system/system_status.dart';

/// Cell side and pitch of the grid.
const double cell = 64;
const double pitch = 76;
const int gridColumns = 4;

enum ModuleSize {
  small(1, 1),
  medium(2, 1),
  large(2, 2),
  wide(4, 1);

  const ModuleSize(this.columns, this.rows);
  final int columns;
  final int rows;

  Size get extent =>
      Size(columns * cell + (columns - 1) * 12, rows * cell + (rows - 1) * 12);
}

@immutable
class CcModule {
  const CcModule(this.id, this.size);

  final String id;
  final ModuleSize size;

  CcModule withSize(ModuleSize size) => CcModule(id, size);

  Map<String, String> toJson() => {'id': id, 'size': size.name};

  @override
  bool operator ==(Object other) =>
      other is CcModule && other.id == id && other.size == size;

  @override
  int get hashCode => Object.hash(id, size);

  @override
  String toString() => '$id:${size.name}';
}

/// The modules Silica offers and the sizes each one can take (the module menu's Size group).
/// The order is the gallery's.
const Map<String, List<ModuleSize>> moduleSizes = {
  'wifi': [ModuleSize.medium],
  'bluetooth': [ModuleSize.medium],
  'nearby': [ModuleSize.medium],
  // The compact 140 × 64 Now playing is not measured: large only for now.
  'media': [ModuleSize.large],
  'dnd': [ModuleSize.medium],
  'dark': [ModuleSize.small, ModuleSize.medium],
  'capture': [ModuleSize.small, ModuleSize.medium],
  'staging': [ModuleSize.small, ModuleSize.medium],
  'mirroring': [ModuleSize.small, ModuleSize.medium],
  'battery': [ModuleSize.small, ModuleSize.medium],
  'display': [ModuleSize.wide],
  'sound': [ModuleSize.wide],
};

/// The reference's default arrangement (INV-A2 grid): Wi-Fi and Bluetooth on the left with
/// Now playing beside them, nearby sharing with the window staging and mirroring tiles, Dark
/// Mode and Screen Capture with Focus, then Display and Sound.
const List<CcModule> defaultModules = [
  CcModule('wifi', ModuleSize.medium),
  CcModule('media', ModuleSize.large),
  CcModule('bluetooth', ModuleSize.medium),
  CcModule('nearby', ModuleSize.medium),
  CcModule('staging', ModuleSize.small),
  CcModule('mirroring', ModuleSize.small),
  CcModule('dark', ModuleSize.small),
  CcModule('capture', ModuleSize.small),
  CcModule('dnd', ModuleSize.medium),
  CcModule('display', ModuleSize.wide),
  CcModule('sound', ModuleSize.wide),
];

/// Places [modules] in order on the four-cell grid: each at the first free spot, row by row.
/// Returns each module's frame relative to the stack's top-left corner.
Map<String, Rect> packModules(List<CcModule> modules) {
  final taken = <(int, int)>{};
  final frames = <String, Rect>{};
  bool fits(int col, int row, ModuleSize size) {
    if (col + size.columns > gridColumns) return false;
    for (var dc = 0; dc < size.columns; dc++) {
      for (var dr = 0; dr < size.rows; dr++) {
        if (taken.contains((col + dc, row + dr))) return false;
      }
    }
    return true;
  }

  for (final m in modules) {
    if (frames.containsKey(m.id)) continue;
    for (var row = 0; ; row++) {
      final col = [
        for (var c = 0; c < gridColumns; c++)
          if (fits(c, row, m.size)) c,
      ].firstOrNull;
      if (col == null) continue;
      for (var dc = 0; dc < m.size.columns; dc++) {
        for (var dr = 0; dr < m.size.rows; dr++) {
          taken.add((col + dc, row + dr));
        }
      }
      frames[m.id] = Offset(col * pitch, row * pitch) & m.size.extent;
      break;
    }
  }
  return frames;
}

/// The modules that can be shown on this machine: the battery only with a battery.
List<CcModule> presentModules(List<CcModule> modules, SystemSnapshot s) => [
  for (final m in modules)
    if (m.id != 'battery' || s.batteryPresent) m,
];

/// The arrangement file: $XDG_CONFIG_HOME/silica/controlcenter.json.
File moduleConfigFile({Map<String, String>? env}) {
  final e = env ?? Platform.environment;
  final config = e['XDG_CONFIG_HOME'] ?? '${e['HOME'] ?? ''}/.config';
  return File('$config/silica/controlcenter.json');
}

/// The user's arrangement, or the default one. Unknown ids and sizes a module can't take
/// are dropped; a missing or broken file gives the default.
List<CcModule> loadModules({Map<String, String>? env}) {
  try {
    final file = moduleConfigFile(env: env);
    if (!file.existsSync()) return defaultModules;
    final json = jsonDecode(file.readAsStringSync());
    final list = json is Map ? json['modules'] : null;
    if (list is! List) return defaultModules;
    final modules = <CcModule>[];
    for (final item in list) {
      if (item is! Map) continue;
      final id = item['id'];
      final sizes = moduleSizes[id];
      if (id is! String || sizes == null) continue;
      final size = ModuleSize.values
          .where((s) => s.name == item['size'])
          .firstOrNull;
      modules.add(
        CcModule(id, size != null && sizes.contains(size) ? size : sizes.first),
      );
    }
    return modules;
  } on Object {
    return defaultModules;
  }
}

/// Saves the arrangement (best effort: a read-only home keeps it for this session only).
void saveModules(List<CcModule> modules, {Map<String, String>? env}) {
  try {
    final file = moduleConfigFile(env: env);
    file.parent.createSync(recursive: true);
    file.writeAsStringSync(
      const JsonEncoder.withIndent('  ').convert({
        'modules': [for (final m in modules) m.toJson()],
      }),
    );
  } on Object {
    // Not saved: the arrangement still applies until the session ends.
  }
}
