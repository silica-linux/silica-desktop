// The bar menus until the global app menu arrives (M7): the logo menu, the app menu and
// the menus with actions Ossido can do by itself (catalog commands).
// Menus without actions (Edit, Help) do not open.

import 'package:silica_ui/silica_ui.dart';

/// The items of a bar menu; `run` runs an Ossido command.
List<MenuEntry> barMenu(
  String name,
  String app,
  void Function(String id) run,
) => switch (name) {
  'logo' => [
    const MenuAction('Informazioni su Silica', enabled: false),
    const MenuDivider(),
    const MenuAction('Impostazioni di sistema…', enabled: false),
    const MenuDivider(),
    MenuAction(
      'Panorama',
      shortcut: '⌃↑',
      onSelected: () => run('panorama.toggle'),
    ),
    MenuAction(
      'Mostra la scrivania',
      shortcut: 'F11',
      onSelected: () => run('desktop.show'),
    ),
    const MenuDivider(),
    MenuAction('Esci dalla sessione…', onSelected: () => run('session.quit')),
  ],
  'app' => [
    MenuAction('Informazioni su $app', enabled: false),
    const MenuDivider(),
    MenuAction(
      'Esci da $app',
      shortcut: '⌘Q',
      onSelected: () => run('app.quit'),
    ),
  ],
  'File' => [
    MenuAction(
      'Chiudi finestra',
      shortcut: '⌘W',
      onSelected: () => run('window.close'),
    ),
  ],
  'Vista' => [
    MenuAction(
      'Schermo intero',
      shortcut: '⌃⌘F',
      onSelected: () => run('window.fullscreen'),
    ),
  ],
  'Finestra' => [
    MenuAction(
      'Riduci',
      shortcut: '⌘M',
      onSelected: () => run('window.minimize'),
    ),
    MenuAction(
      'Schermo intero',
      shortcut: '⌃⌘F',
      onSelected: () => run('window.fullscreen'),
    ),
    const MenuDivider(),
    MenuAction(
      "Finestre dell'app",
      shortcut: '⌃↓',
      onSelected: () => run('panorama.app'),
    ),
    MenuAction(
      'Spazio successivo',
      shortcut: '⌃→',
      onSelected: () => run('space.next'),
    ),
    MenuAction(
      'Spazio precedente',
      shortcut: '⌃←',
      onSelected: () => run('space.previous'),
    ),
  ],
  _ => const [],
};
