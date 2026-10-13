// Le icone delle app dai temi del sistema (specifica freedesktop Icon Theme, semplificata):
// file dell'utente, non asset del repository.

import 'dart:io';

import 'desktop_entries.dart';

/// Temi da provare, in ordine: quello di Ubuntu, Adwaita, poi hicolor (il ripiego).
const List<String> iconThemes = ['Yaru', 'Adwaita', 'hicolor'];

/// Dimensioni preferite per un'icona del Dock (48 pt, fino a 2× con l'ingrandimento).
const List<String> iconSizes = [
  'scalable',
  '256x256',
  '128x128',
  '512x512',
  '96x96',
  '64x64',
  '48x48',
];

/// Il file dell'icona `name` (app o luogo, come la cartella o il cestino), o null.
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
