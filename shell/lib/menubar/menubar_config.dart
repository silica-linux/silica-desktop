// Which optional status items the bar shows (Settings › Menu Bar), in
// `$XDG_CONFIG_HOME/silica/menubar.json`:
//
//   {"sound": "always" | "active" | "never", "bluetooth": true, "battery": true,
//    "batteryPercent": false}
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
  });

  final SoundItem sound;
  final bool bluetooth;
  final bool battery;
  final bool batteryPercent;

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
    );
  }

  @override
  bool operator ==(Object other) =>
      other is MenubarConfig &&
      other.sound == sound &&
      other.bluetooth == bluetooth &&
      other.battery == battery &&
      other.batteryPercent == batteryPercent;

  @override
  int get hashCode => Object.hash(sound, bluetooth, battery, batteryPercent);
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
