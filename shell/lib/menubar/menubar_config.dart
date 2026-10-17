// Which optional status items the bar shows (Settings › Menu Bar), in
// `$XDG_CONFIG_HOME/silica/menubar.json`:
//
//   {"sound": "always" | "active" | "never", "bluetooth": true, "battery": true,
//    "batteryPercent": false, "focus": false}
//
// Defaults as on the reference (menubar.md › Status items, INV-A1): Bluetooth not in the bar,
// Sound "when active", battery shown when there is one. The file is watched: Settings changes
// apply at once.

import 'dart:async';
import 'dart:convert';
import 'dart:io';

import 'package:flutter/foundation.dart';

enum SoundItem { always, active, never }

@immutable
class MenubarConfig {
  const MenubarConfig({
    this.sound = SoundItem.active,
    this.bluetooth = false,
    this.battery = true,
    this.batteryPercent = false,
    this.focus = false,
  });

  final SoundItem sound;
  final bool bluetooth;
  final bool battery;
  final bool batteryPercent;

  /// The Focus moon always shown (Control Center › Copy to Menu Bar); otherwise only while a
  /// Focus is on.
  final bool focus;

  factory MenubarConfig.fromJson(Object? json) {
    if (json is! Map) return const MenubarConfig();
    final sound = switch (json['sound']) {
      'always' => SoundItem.always,
      'never' => SoundItem.never,
      _ => SoundItem.active,
    };
    bool flag(String key, bool fallback) =>
        json[key] is bool ? json[key] as bool : fallback;
    return MenubarConfig(
      sound: sound,
      bluetooth: flag('bluetooth', false),
      battery: flag('battery', true),
      batteryPercent: flag('batteryPercent', false),
      focus: flag('focus', false),
    );
  }

  @override
  bool operator ==(Object other) =>
      other is MenubarConfig &&
      other.sound == sound &&
      other.bluetooth == bluetooth &&
      other.battery == battery &&
      other.batteryPercent == batteryPercent &&
      other.focus == focus;

  @override
  int get hashCode =>
      Object.hash(sound, bluetooth, battery, batteryPercent, focus);
}

/// The configuration file's path.
String menubarConfigPath([Map<String, String>? env]) {
  final e = env ?? Platform.environment;
  final config = e['XDG_CONFIG_HOME'] ?? '${e['HOME'] ?? ''}/.config';
  return '$config/silica/menubar.json';
}

MenubarConfig readMenubarConfig([Map<String, String>? env]) {
  try {
    return MenubarConfig.fromJson(
      jsonDecode(File(menubarConfigPath(env)).readAsStringSync()),
    );
  } on FileSystemException {
    return const MenubarConfig();
  } on FormatException {
    return const MenubarConfig();
  }
}

/// The Control Center modules that have a status item in the bar, and which ones the bar shows
/// now (the module menu's Copy to / Remove from Menu Bar, silica-desktop#145). Wi-Fi is always
/// in the bar and cannot be removed.
const menubarModules = {'bluetooth', 'sound', 'dnd', 'battery'};

/// Whether the bar shows the status item of Control Center module [id] under [config].
bool menubarShows(MenubarConfig config, String id) => switch (id) {
  'bluetooth' => config.bluetooth,
  'sound' => config.sound == SoundItem.always,
  'dnd' => config.focus,
  'battery' => config.battery,
  _ => false,
};

/// Shows or hides the status item of module [id] by rewriting `menubar.json` (the other keys
/// stay as they are, the watcher applies the change). Returns false when it could not write.
bool setMenubarModule(String id, bool shown, [Map<String, String>? env]) {
  final key = switch (id) {
    'bluetooth' => 'bluetooth',
    'sound' => 'sound',
    'dnd' => 'focus',
    'battery' => 'battery',
    _ => null,
  };
  if (key == null) return false;
  final file = File(menubarConfigPath(env));
  try {
    var json = <String, Object?>{};
    try {
      final read = jsonDecode(file.readAsStringSync());
      if (read is Map) json = Map<String, Object?>.from(read);
    } on FileSystemException {
      // No file yet: the defaults plus this key.
    } on FormatException {
      // Unreadable: start again from the defaults.
    }
    json[key] = key == 'sound' ? (shown ? 'always' : 'never') : shown;
    file.parent.createSync(recursive: true);
    file.writeAsStringSync(
      '${const JsonEncoder.withIndent('  ').convert(json)}\n',
    );
    return true;
  } on FileSystemException {
    return false;
  }
}

/// The configuration, re-read when its folder changes.
class MenubarConfigWatcher extends ValueNotifier<MenubarConfig> {
  MenubarConfigWatcher([Map<String, String>? env])
    : _env = env,
      super(readMenubarConfig(env)) {
    final dir = File(menubarConfigPath(env)).parent;
    try {
      _sub = dir.watch().listen(
        (_) => value = readMenubarConfig(_env),
        onError: (Object _) {},
      );
    } on FileSystemException {
      // No folder yet: the defaults stay until the next session.
    }
  }

  final Map<String, String>? _env;
  StreamSubscription<FileSystemEvent>? _sub;

  @override
  void dispose() {
    _sub?.cancel();
    super.dispose();
  }
}
