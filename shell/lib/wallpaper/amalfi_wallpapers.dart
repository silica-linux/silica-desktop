// The Amalfi wallpapers of silica-wallpapers (CC BY-SA 4.0): `<data dir>/silica/wallpapers/
// amalfi/wallpapers.json` with `<id>/light.png` and `<id>/dark.png` (square, 2880) and the
// 640 px previews. They are also compiled into `silica.assets` (docs/assets-format.md); the
// shell reads the files, like the icons.

import 'dart:convert';
import 'dart:io';

import 'package:silica_ui/silica_ui.dart' show xdgDataDirs;

/// The default wallpaper (user decision 2026-10-16).
const String defaultWallpaperId = 'ravello';

/// One wallpaper of the set.
class AmalfiWallpaperInfo {
  const AmalfiWallpaperInfo({
    required this.id,
    required this.title,
    required this.light,
    required this.dark,
    this.lightPreview,
    this.darkPreview,
  });

  final String id;
  final String title;

  /// The full images, absolute paths.
  final String light;
  final String dark;
  final String? lightPreview;
  final String? darkPreview;

  String image({required bool dark}) => dark ? this.dark : light;
}

/// The folders that may hold the `amalfi/` set: `SILICA_WALLPAPERS`, the data folders'
/// `silica/wallpapers`, and a silica-wallpapers checkout in `~/silica`.
List<String> wallpaperRoots(Map<String, String> env) => [
  if (env['SILICA_WALLPAPERS'] case final dir? when dir.isNotEmpty) dir,
  for (final dir in xdgDataDirs(env)) '$dir/silica/wallpapers',
  if (env['HOME'] case final home? when home.isNotEmpty)
    '$home/silica/silica-wallpapers',
];

/// The wallpapers of the Amalfi set, empty when it is not installed.
List<AmalfiWallpaperInfo> loadAmalfiWallpapers({Map<String, String>? env}) {
  for (final root in wallpaperRoots(env ?? Platform.environment)) {
    final set = '$root/amalfi';
    final manifest = File('$set/wallpapers.json');
    try {
      if (!manifest.existsSync()) continue;
      final json = jsonDecode(manifest.readAsStringSync());
      final list = json is Map ? json['wallpapers'] : null;
      if (list is! List) continue;
      final out = <AmalfiWallpaperInfo>[];
      for (final item in list) {
        if (item is! Map) continue;
        final id = item['id'];
        final variants = item['variants'];
        if (id is! String || variants is! Map) continue;
        String? file(String theme) {
          final v = variants[theme];
          final f = v is Map ? v['file'] : null;
          if (f is! String || f.contains('..')) return null;
          final path = '$set/$f';
          return File(path).existsSync() ? path : null;
        }

        String? preview(String theme) {
          final path = '$set/previews/$id-$theme.jpg';
          return File(path).existsSync() ? path : null;
        }

        final light = file('light');
        final dark = file('dark');
        if (light == null && dark == null) continue;
        out.add(
          AmalfiWallpaperInfo(
            id: id,
            title: item['title'] is String ? item['title'] as String : id,
            light: light ?? dark!,
            dark: dark ?? light!,
            lightPreview: preview('light'),
            darkPreview: preview('dark'),
          ),
        );
      }
      if (out.isNotEmpty) return out;
    } on FileSystemException {
      continue;
    } on FormatException {
      continue;
    }
  }
  return const [];
}
