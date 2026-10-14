// The bar menus until the global app menu arrives (M7): the app menu and the menus with
// actions Ossido can do by itself (catalog commands). The logo menu is in logo_menu.dart.
// Menus without actions (Edit, until apps publish their own menus: #24, #25) are not shown,
// so no title in the bar is dead. Help opens Silica's documentation.

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

/// Where Help › Silica Help leads.
const String silicaHelpUrl = 'https://github.com/silica-linux/silica-desktop#readme';

/// The items of a bar menu (by id); `run` runs an Ossido command, `openUrl` opens a link.
List<MenuEntry> barMenu(
  String name,
  String app,
  void Function(String id) run, {
  AppLocalizations? texts,
  void Function(String url)? openUrl,
}) {
  final l = texts ?? englishTexts;
  return switch (name) {
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
    'help' => [
      MenuAction(
        l.silicaHelp,
        enabled: openUrl != null,
        onSelected: () => openUrl?.call(silicaHelpUrl),
      ),
    ],
    _ => const [],
  };
}
