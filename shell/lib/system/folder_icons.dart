// Full folders: a folder with at least one visible item is drawn with the `full` variant of
// its silica-icons icon (a sheet between the back and front panels). The folder's symbol and
// tint stay dynamic on top. The variant is picked here with the public silica_ui types, so it
// works before silica_ui itself offers a `full` flag.

import 'dart:convert';
import 'dart:io';

import 'package:silica_ui/silica_ui.dart';

/// Per folder: its modification time when it was listed, and whether it has a visible item.
/// A folder's modification time changes when an item is added, removed or renamed, so one
/// `stat` per query tells whether the cached answer still holds; the listing runs again only
/// then, and it is not recursive.
final Map<String, (DateTime, bool)> _hasItems = {};

/// The folder at [path] contains at least one item. Hidden files (names starting with a dot)
/// do not count. A folder that cannot be read is empty.
bool folderHasItems(String path) {
  try {
    final directory = Directory(path);
    final changed = directory.statSync().modified;
    final cached = _hasItems[path];
    if (cached != null && cached.$1 == changed) return cached.$2;
    var found = false;
    for (final item in directory.listSync(followLinks: false)) {
      final slash = item.path.lastIndexOf('/');
      if (!item.path.substring(slash + 1).startsWith('.')) {
        found = true;
        break;
      }
    }
    if (_hasItems.length > 2000) _hasItems.clear();
    _hasItems[path] = (changed, found);
    return found;
  } on FileSystemException {
    return false;
  }
}

/// The `full` variants already read, per manifest folder (null: the icon has none).
final Map<String, LayeredIcon?> _fullIcons = {};

final RegExp _svgName = RegExp(r'^[A-Za-z0-9][A-Za-z0-9._-]{0,63}\.svg$');

/// [icon] with the artwork of its `full` and `full-dark` variants (`variants` of its
/// `icon.json`); [icon] itself if it has none. Id, kind and dynamic content are kept.
LayeredIcon fullFolderVariant(LayeredIcon icon) {
  if (icon.layers.length != 1) return icon;
  final path = icon.layers.first.path;
  final slash = path.lastIndexOf('/');
  if (slash < 0) return icon;
  final dir = path.substring(0, slash);
  final key = '$dir\u0000${icon.id}';
  if (_fullIcons.containsKey(key)) return _fullIcons[key] ?? icon;
  LayeredIcon? full;
  try {
    final json = jsonDecode(File('$dir/icon.json').readAsStringSync());
    final variants = json is Map ? json['variants'] : null;
    String? artwork(String name) {
      final variant = variants is Map ? variants[name] : null;
      final file = variant is Map ? variant['artwork'] : null;
      if (file is! String || !_svgName.hasMatch(file)) return null;
      return File('$dir/$file').existsSync() ? '$dir/$file' : null;
    }

    final light = artwork('full');
    if (light != null) {
      full = LayeredIcon(
        id: icon.id,
        kind: icon.kind,
        layers: [
          IconLayer(
            light,
            darkPath: artwork('full-dark'),
            shadow: icon.layers.first.shadow,
          ),
        ],
        dynamic: icon.dynamic,
      );
    }
  } on FileSystemException {
    full = null;
  } on FormatException {
    full = null;
  }
  _fullIcons[key] = full;
  return full ?? icon;
}

/// [folder] drawn full: same symbol and tint, the `full` artwork.
FolderIcon fullFolder(FolderIcon folder) => FolderIcon(
  fullFolderVariant(folder.icon),
  symbol: folder.symbol,
  tint: folder.tint,
);
