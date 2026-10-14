// The desktop context menu (SH-MB-15; misc.md › Desktop context menu): a right click on
// the wallpaper. The wallpaper sits below the windows, so it only announces the click
// (`desktopmenu` = "x y") and the menu bar, on the top layer, shows the menu with its
// top-left corner at the click and the first item under the pointer.
//
// Items as in the reference: New Folder, Paste Item, Get Info, Change Wallpaper…, Edit
// Widgets…, Use Stacks, Sort By ›, Show View Options. The ones that need desktop icons,
// widgets or the clipboard of files (not there yet) are disabled; the system's "Services"
// section is left out.

import 'dart:io';

import 'package:flutter/widgets.dart';
import 'package:silica_ui/silica_ui.dart';

import '../l10n/l10n.dart';

/// The menu's top edge is 11 pt above the click: the first item is under the pointer.
const double desktopMenuAbove = 11;

/// The desktop folder: `XDG_DESKTOP_DIR` of `user-dirs.dirs`, else `~/Desktop`.
String desktopFolder({Map<String, String>? env, String? userDirs}) {
  final e = env ?? Platform.environment;
  final home = e['HOME'] ?? '';
  String? text = userDirs;
  if (text == null) {
    try {
      text = File(
        '${e['XDG_CONFIG_HOME'] ?? '$home/.config'}/user-dirs.dirs',
      ).readAsStringSync();
    } on FileSystemException {
      text = null;
    }
  }
  final line = RegExp(
    r'^XDG_DESKTOP_DIR="([^"]*)"',
    multiLine: true,
  ).firstMatch(text ?? '');
  final path = line?.group(1)?.replaceAll(r'$HOME', home);
  return path == null || path.isEmpty ? '$home/Desktop' : path;
}

/// Creates "<name>", or "<name> 2", "<name> 3"… in `folder`; the path, or null.
String? newFolder(String folder, String name) {
  try {
    Directory(folder).createSync(recursive: true);
    for (var i = 1; i < 1000; i++) {
      final path = i == 1 ? '$folder/$name' : '$folder/$name $i';
      if (FileSystemEntity.typeSync(path) == FileSystemEntityType.notFound) {
        Directory(path).createSync();
        return path;
      }
    }
  } on FileSystemException {
    return null;
  }
  return null;
}

List<MenuEntry> desktopMenu({
  required AppLocalizations l,
  required VoidCallback onNewFolder,
  VoidCallback? onChangeWallpaper,
}) => [
  MenuAction(l.desktopNewFolder, onSelected: onNewFolder),
  MenuAction(l.desktopPaste, enabled: false),
  const MenuDivider(),
  MenuAction(l.desktopGetInfo, enabled: false),
  MenuAction(
    l.desktopChangeWallpaper,
    enabled: onChangeWallpaper != null,
    onSelected: onChangeWallpaper,
  ),
  MenuAction(l.desktopEditWidgets, enabled: false),
  const MenuDivider(),
  MenuAction(l.desktopUseStacks, enabled: false),
  MenuAction(
    l.desktopSortBy,
    submenu: [
      MenuAction(l.desktopSortNone, checked: true, enabled: false),
      MenuAction(l.desktopSortName, enabled: false),
      MenuAction(l.desktopSortKind, enabled: false),
      MenuAction(l.desktopSortModified, enabled: false),
    ],
  ),
  MenuAction(l.desktopViewOptions, enabled: false),
];
