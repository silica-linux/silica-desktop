// La shell di Silica: un componente per processo, scelto dal primo argomento di main
// (silica-embedder --dart-arg dock). Oggi: dock (le app vere), dock-demo (segnaposto),
// menubar, wallpaper, faro, notifications, controlcenter, switcher.

import 'package:flutter/widgets.dart';

import 'controlcenter/controlcenter.dart';
import 'dock/dock.dart';
import 'dock/model.dart';
import 'faro/faro.dart';
import 'menubar/menubar.dart';
import 'notifications/notifications.dart';
import 'notifications/service.dart';
import 'system/desktop_entries.dart';
import 'system/shell_channel.dart';
import 'switcher/switcher.dart';
import 'system/system_status.dart';
import 'wallpaper/wallpaper.dart';

/// Segnaposto del Dock per i test e il confronto con le riprese: 12 app, separatore,
/// cartella e cestino.
const List<Item> placeholderItems = [
  Item.app('File', running: true),
  Item.app('Navigatore', running: true),
  Item.app('Posta'),
  Item.app('Messaggi'),
  Item.app('Calendario'),
  Item.app('Note'),
  Item.app('Foto'),
  Item.app('Musica'),
  Item.app('Mappe'),
  Item.app('Editor'),
  Item.app('Terminale', running: true),
  Item.app('Impostazioni'),
  Item.separator(),
  Item.app('Download'),
  Item.app('Cestino'),
];

void main(List<String> args) {
  final component = args.isEmpty ? 'dock' : args.first;
  runApp(ShellComponent(component: component));
}

class ShellComponent extends StatelessWidget {
  const ShellComponent({super.key, required this.component});

  final String component;

  @override
  Widget build(BuildContext context) {
    final Widget child = switch (component) {
      'dock-demo' => const Dock(items: placeholderItems),
      'menubar' => MenuBar(
        entries: loadDesktopEntries(),
        onSearch: () => ShellChannel.instance.execute('faro.toggle'),
        onClock: () => ShellChannel.instance.execute('notifications.toggle'),
        onControls: () => ShellChannel.instance.execute('controlcenter.toggle'),
        system: DBusSystem(),
      ),
      'wallpaper' => Wallpaper(choice: loadWallpaperChoice()),
      'faro' => Faro(entries: loadDesktopEntries()),
      'notifications' => NotificationsLayer(
        source: DBusNotifications(),
        system: DBusSystem(),
      ),
      'controlcenter' => ControlCenter(system: DBusSystem()),
      'switcher' => AppSwitcher(entries: loadDesktopEntries()),
      _ => const LiveDock(),
    };
    return Directionality(textDirection: TextDirection.ltr, child: child);
  }
}
