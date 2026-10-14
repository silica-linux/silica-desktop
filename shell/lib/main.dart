// The Silica shell: one component per process, chosen by the first argument of main
// (silica-embedder --dart-arg dock). Today: dock (the real apps), dock-demo (placeholders),
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
import 'forcequit/force_quit.dart';
import 'screenshot/screenshot.dart';
import 'switcher/switcher.dart';
import 'system/system_status.dart';
import 'wallpaper/wallpaper.dart';

/// Dock placeholders for tests and for comparison with the recordings: 12 apps, separator,
/// folder and trash.
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
      // The app switcher never takes input: the same layer hosts the screenshots.
      'switcher' => Stack(
        children: [
          AppSwitcher(entries: loadDesktopEntries()),
          const ScreenshotLayer(),
          ForceQuitLayer(entries: loadDesktopEntries()),
        ],
      ),
      _ => const LiveDock(),
    };
    return Directionality(textDirection: TextDirection.ltr, child: child);
  }
}
