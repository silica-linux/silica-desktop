// App icons from the system themes (freedesktop Icon Theme specification, simplified): the
// user's files, not repository assets.

import 'dart:io';

import 'desktop_entries.dart';

/// Themes to try, in order: Ubuntu's, Adwaita, then hicolor (the fallback).
const List<String> iconThemes = ['Yaru', 'Adwaita', 'hicolor'];

/// Preferred sizes for a Dock icon (48 pt, up to 2× with magnification).
const List<String> iconSizes = [
  'scalable',
  '256x256',
  '128x128',
  '512x512',
  '96x96',
  '64x64',
  '48x48',
];

/// The file of icon `name` (an app or a place, like the folder or the trash), or null.
String? findIcon(String? name, {Map<String, String>? env}) {
  if (name == null || name.isEmpty) return null;
  if (name.startsWith('/')) return File(name).existsSync() ? name : null;
  final environment = env ?? Platform.environment;
  final home = environment['HOME'] ?? '';
  final bases = [
    '$home/.icons',
    for (final dir in xdgDataDirs(environment)) '$dir/icons',
  ];
  for (final theme in iconThemes) {
    for (final base in bases) {
      for (final size in iconSizes) {
        for (final category in ['apps', 'places']) {
          for (final ext in size == 'scalable' ? ['svg'] : ['png', 'svg']) {
            final path = '$base/$theme/$size/$category/$name.$ext';
            if (File(path).existsSync()) return path;
          }
        }
      }
    }
  }
  for (final ext in ['png', 'svg']) {
    final path = '/usr/share/pixmaps/$name.$ext';
    if (File(path).existsSync()) return path;
  }
  return null;
}
