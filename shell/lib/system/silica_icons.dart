// Which silica-icons icon an app gets: only the one declared by its .desktop entry.
// The rest (manifests, special folders, drawing) is in silica_ui (`SilicaIcon`).

import 'dart:io';

import 'package:silica_ui/silica_ui.dart';

import 'desktop_entries.dart';

/// The Silica icon of an app: only the one the entry declares with `X-Silica-Icon` (Silica's
/// own apps); the others keep the icon of their theme.
///
/// The value is either an absolute path, the app's own icon folder (the `icon/` folder of its
/// bundle: `icon.json`, layers, rendered PNGs; docs/assets-format.md "App icons"), or a plain
/// name looked up in the shell's icons (`silica.assets` / the icons folder). A path whose folder
/// has no valid icon gives null (the entry's `Icon=` is used), never the name lookup.
LayeredIcon? silicaIconFor(DesktopEntry entry, {Map<String, String>? env}) {
  final id = entry.silicaIcon;
  if (id == null) return null;
  if (id.startsWith('/')) return appIconFromFolder(id);
  return findSilicaIcon(id, env: env);
}

final Map<String, LayeredIcon?> _folderCache = {};

/// The icon in the folder [path] (an absolute path holding `icon.json`), or null. The folder is
/// read as it is: the rendered PNG of the current theme and the dynamic layers (clock hands,
/// calendar) are drawn by silica_ui exactly as for an icon from the icons folder. Valid icons are
/// cached; a missing folder is looked up again, since the app may be installed later.
LayeredIcon? appIconFromFolder(String path) {
  if (!path.startsWith('/') || path.split('/').contains('..')) return null;
  final dir = path.length > 1 && path.endsWith('/')
      ? path.substring(0, path.length - 1)
      : path;
  if (_folderCache[dir] case final cached?) return cached;
  final manifest = File('$dir/icon.json');
  try {
    if (!manifest.existsSync()) return null;
    final icon = parseManifest(dir, manifest.readAsStringSync());
    if (icon != null) _folderCache[dir] = icon;
    return icon;
  } on FileSystemException {
    return null;
  }
}
