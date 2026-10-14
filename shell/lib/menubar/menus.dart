// The bar menus until the global app menu arrives (M7): the logo menu, the app menu and
// the menus with actions Ossido can do by itself (catalog commands).
// Menus without actions (Edit, Help) do not open.

import 'package:silica_ui/silica_ui.dart';

import '../l10n/l10n.dart';

/// The shown title of a generic bar menu id.
String menuTitle(AppLocalizations l, String id) => switch (id) {
  'file' => l.menuFile,
  'edit' => l.menuEdit,
  'view' => l.menuView,
  'window' => l.menuWindow,
  'help' => l.menuHelp,
  _ => id,
};

/// The items of a bar menu (by id); `run` runs an Ossido command.
List<MenuEntry> barMenu(
  String name,
  String app,
  void Function(String id) run, {
  AppLocalizations? texts,
}) {
  final l = texts ?? englishTexts;
  return switch (name) {
    'logo' => [
      MenuAction(l.aboutSilica, enabled: false),
      const MenuDivider(),
      MenuAction(l.systemSettings, enabled: false),
      const MenuDivider(),
      MenuAction(
        l.panorama,
        shortcut: '⌃↑',
        onSelected: () => run('panorama.toggle'),
      ),
      MenuAction(
        l.showDesktop,
        shortcut: 'F11',
        onSelected: () => run('desktop.show'),
      ),
      const MenuDivider(),
      MenuAction(l.logOut, onSelected: () => run('session.quit')),
    ],
    'app' => [
      MenuAction(l.aboutApp(app), enabled: false),
      const MenuDivider(),
      MenuAction(
        l.quitApp(app),
        shortcut: '⌘Q',
        onSelected: () => run('app.quit'),
      ),
    ],
    'file' => [
      MenuAction(
        l.closeWindow,
        shortcut: '⌘W',
        onSelected: () => run('window.close'),
      ),
    ],
    'view' => [
      MenuAction(
        l.fullScreen,
        shortcut: '⌃⌘F',
        onSelected: () => run('window.fullscreen'),
      ),
    ],
    'window' => [
      MenuAction(
        l.minimize,
        shortcut: '⌘M',
        onSelected: () => run('window.minimize'),
      ),
      MenuAction(
        l.fullScreen,
        shortcut: '⌃⌘F',
        onSelected: () => run('window.fullscreen'),
      ),
      const MenuDivider(),
      MenuAction(
        l.appWindows,
        shortcut: '⌃↓',
        onSelected: () => run('panorama.app'),
      ),
      MenuAction(
        l.nextSpace,
        shortcut: '⌃→',
        onSelected: () => run('space.next'),
      ),
      MenuAction(
        l.previousSpace,
        shortcut: '⌃←',
        onSelected: () => run('space.previous'),
      ),
    ],
    _ => const [],
  };
}
