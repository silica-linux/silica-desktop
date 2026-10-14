// The Silica shell: one component per process, chosen by the first argument of main
// (silica-embedder --dart-arg dock). Today: dock (the real apps), dock-demo (placeholders),
// menubar, wallpaper, faro, notifications, controlcenter, switcher.

import 'dart:io';

import 'package:flutter/scheduler.dart' show timeDilation;
import 'package:flutter/widgets.dart';

import 'controlcenter/controlcenter.dart';
import 'l10n/l10n.dart';
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
import 'system/media.dart';
import 'wallpaper/wallpaper.dart';

/// Dock placeholders for tests and for comparison with the recordings: 12 apps, separator,
/// folder and trash.
const List<Item> placeholderItems = [
  Item.app('Files', running: true),
  Item.app('Browser', running: true),
  Item.app('Mail'),
  Item.app('Messages'),
  Item.app('Calendar'),
  Item.app('Notes'),
  Item.app('Photos'),
  Item.app('Music'),
  Item.app('Maps'),
  Item.app('Editor'),
  Item.app('Terminal', running: true),
  Item.app('Settings'),
  Item.separator(),
  Item.app('Downloads'),
  Item.app('Trash', trash: true),
];

void main(List<String> args) {
  final component = args.isEmpty ? 'dock' : args.first;
  // Debug only: SILICA_TIME_DILATION=10 slows every animation of the shell tenfold, to look
  // at transitions frame by frame (docs/vm.md › Looking at animations).
  final dilation = double.tryParse(
    Platform.environment['SILICA_TIME_DILATION'] ?? '',
  );
  if (dilation != null && dilation > 0) timeDilation = dilation;
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
        entries: loadDesktopEntries(locale: systemLanguage()),
        onSearch: () => ShellChannel.instance.execute('faro.toggle'),
        onClock: () => ShellChannel.instance.execute('notifications.toggle'),
        onControls: () => ShellChannel.instance.execute('controlcenter.toggle'),
        system: DBusSystem(),
        media: MediaPlayers.instance,
      ),
      'wallpaper' => Wallpaper(choice: loadWallpaperChoice()),
      'faro' => Faro(entries: loadDesktopEntries(locale: systemLanguage())),
      'notifications' => NotificationsLayer(
        source: DBusNotifications(),
        system: DBusSystem(),
      ),
      'controlcenter' => ControlCenter(system: DBusSystem()),
      // The app switcher never takes input: the same layer hosts the screenshots.
      'switcher' => Stack(
        children: [
          AppSwitcher(entries: loadDesktopEntries(locale: systemLanguage())),
          const ScreenshotLayer(),
          ForceQuitLayer(entries: loadDesktopEntries(locale: systemLanguage())),
        ],
      ),
      _ => const LiveDock(),
    };
    return ShellLocalizations(
      child: Directionality(textDirection: TextDirection.ltr, child: child),
    );
  }
}
