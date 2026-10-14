// The desktop context menu (SH-MB-15; misc.md › Desktop context menu): a right click on
// the wallpaper. The wallpaper sits below the windows, so it only announces the click
// (`desktopmenu` = "x y") and the menu bar, on the top layer, shows the menu with its
// top-left corner at the click and the first item under the pointer.
//
// Items as in the reference: New Folder, Paste Item, Get Info, Change Wallpaper…, Edit
// Widgets…, Use Stacks, Sort By ›, Show View Options; the system's "Services" section is left
// out. When the file manager draws the desktop (#147) it announces the click itself
// ("x y desktop" or "x y items") and runs the items it owns, sent back as `desktop.action`;
// without it (the wallpaper's "x y") the items that need desktop icons are disabled.

import 'dart:convert';
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

/// Who asked for the desktop menu.
enum DesktopMenuKind {
  /// The wallpaper, with no file manager drawing the desktop: the shell's own menu.
  wallpaper,

  /// The file manager's desktop (#147), on empty desktop.
  desktop,

  /// The file manager's desktop, on its selected icons.
  items,
}

/// A request for the desktop menu: `desktopmenu` = "x y" from the wallpaper, "x y desktop" or
/// "x y items" from the file manager's desktop; null when the menu closes.
(Offset, DesktopMenuKind)? parseDesktopMenu(String value) {
  final parts = value.split(' ');
  if (parts.length < 2 || parts.length > 3) return null;
  final x = double.tryParse(parts[0]);
  final y = double.tryParse(parts[1]);
  if (x == null || y == null || !x.isFinite || !y.isFinite) return null;
  final kind = parts.length == 2
      ? DesktopMenuKind.wallpaper
      : switch (parts[2]) {
          'desktop' => DesktopMenuKind.desktop,
          'items' => DesktopMenuKind.items,
          _ => null,
        };
  return kind == null ? null : (Offset(x, y), kind);
}

/// The sort orders of the desktop, as the file manager names them.
const List<String> desktopSorts = ['none', 'name', 'kind', 'modified', 'size'];

/// What the file manager's desktop announces about itself (`desktop.state`, JSON): the menu
/// shows its choices checked and enables what applies.
@immutable
class DesktopOwnerState {
  const DesktopOwnerState({
    this.stacks = false,
    this.sort = 'none',
    this.selected = 0,
  });

  /// Use Stacks is on.
  final bool stacks;

  /// One of [desktopSorts].
  final String sort;

  /// How many icons are selected.
  final int selected;

  static DesktopOwnerState parse(String json) {
    try {
      final value = jsonDecode(json);
      if (value is! Map) return const DesktopOwnerState();
      final sort = value['sort'];
      final selected = value['selected'];
      return DesktopOwnerState(
        stacks: value['stacks'] == true,
        sort: sort is String && desktopSorts.contains(sort) ? sort : 'none',
        selected: selected is int && selected >= 0 ? selected : 0,
      );
    } on FormatException {
      return const DesktopOwnerState();
    }
  }
}

/// The file manager's desktop menu on empty desktop: every item is an action the file manager
/// runs (`onAction` with its id, sent back as `desktop.action`), except Change Wallpaper….
List<MenuEntry> ownerDesktopMenu({
  required AppLocalizations l,
  required DesktopOwnerState state,
  required ValueChanged<String> onAction,
  VoidCallback? onChangeWallpaper,
}) {
  final free = state.sort == 'none' && !state.stacks;
  return [
    MenuAction(l.desktopNewFolder, onSelected: () => onAction('newFolder')),
    // No clipboard of files yet (silica-desktop#150).
    MenuAction(l.desktopPaste, enabled: false),
    const MenuDivider(),
    // Get Info needs a window of the file manager (#146).
    MenuAction(l.desktopGetInfo, enabled: false),
    MenuAction(
      l.desktopChangeWallpaper,
      enabled: onChangeWallpaper != null,
      onSelected: onChangeWallpaper,
    ),
    MenuAction(l.desktopEditWidgets, enabled: false),
    const MenuDivider(),
    MenuAction(
      l.desktopUseStacks,
      checked: state.stacks,
      onSelected: () => onAction('stacks'),
    ),
    MenuAction(
      l.desktopSortBy,
      submenu: [
        for (final (sort, label) in [
          ('none', l.desktopSortNone),
          ('name', l.desktopSortName),
          ('kind', l.desktopSortKind),
          ('modified', l.desktopSortModified),
          ('size', l.desktopSortSize),
        ])
          MenuAction(
            label,
            checked: state.sort == sort,
            onSelected: () => onAction('sort:$sort'),
          ),
      ],
    ),
    MenuAction(
      l.desktopCleanUp,
      enabled: free,
      onSelected: () => onAction('cleanUp'),
    ),
    MenuAction(
      l.desktopCleanUpBy,
      enabled: !state.stacks,
      submenu: [
        for (final (sort, label) in [
          ('name', l.desktopSortName),
          ('kind', l.desktopSortKind),
          ('modified', l.desktopSortModified),
          ('size', l.desktopSortSize),
        ])
          MenuAction(label, onSelected: () => onAction('cleanUpBy:$sort')),
      ],
    ),
    // View options need a panel of the file manager (#146).
    MenuAction(l.desktopViewOptions, enabled: false),
  ];
}

/// The file manager's desktop menu on its selected icons.
List<MenuEntry> ownerItemsMenu({
  required AppLocalizations l,
  required DesktopOwnerState state,
  required ValueChanged<String> onAction,
}) => [
  MenuAction(l.desktopOpen, onSelected: () => onAction('open')),
  const MenuDivider(),
  MenuAction(l.desktopMoveToTrash, onSelected: () => onAction('trash')),
  const MenuDivider(),
  // Get Info and the quick preview need a window of the file manager (#146).
  MenuAction(l.desktopGetInfo, enabled: false),
  MenuAction(
    l.desktopRename,
    enabled: state.selected == 1,
    onSelected: () => onAction('rename'),
  ),
  MenuAction(l.desktopDuplicate, onSelected: () => onAction('duplicate')),
  MenuAction(l.desktopQuickPreview, enabled: false),
];

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
