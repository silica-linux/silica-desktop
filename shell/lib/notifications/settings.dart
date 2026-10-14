// Per-app notification settings ($XDG_CONFIG_HOME/silica/notifications.json, written by the
// Settings app's Notifications pane):
//
//   {"apps": {"<desktop entry or app name>": {"style": "banners" | "alerts" | "none"}}}
//
// - banners (default): the banner leaves by itself after ~5 s;
// - alerts: the banner stays until the user acts on it (SH-NC-05; the reference style was not
//   measured: it behaves like a critical notification, with the × always visible,
//   provisional);
// - none: no banner, the notification goes straight to the Center.

import 'dart:convert';
import 'dart:io';

import 'model.dart';

enum NotificationStyle { banners, alerts, none }

class NotificationSettings {
  const NotificationSettings([this.styles = const {}]);

  /// Style by group key (desktop entry or app name).
  final Map<String, NotificationStyle> styles;

  NotificationStyle styleOf(ShellNotification n) =>
      styles[n.group] ?? styles[n.appName] ?? NotificationStyle.banners;

  static File file({Map<String, String>? env}) {
    final e = env ?? Platform.environment;
    final config = e['XDG_CONFIG_HOME'] ?? '${e['HOME'] ?? ''}/.config';
    return File('$config/silica/notifications.json');
  }

  /// The settings on disk; the defaults when the file is missing or unreadable.
  static NotificationSettings read({Map<String, String>? env}) {
    try {
      final f = file(env: env);
      if (!f.existsSync()) return const NotificationSettings();
      return parse(f.readAsStringSync());
    } on FileSystemException {
      return const NotificationSettings();
    }
  }

  static NotificationSettings parse(String text) {
    Object? json;
    try {
      json = jsonDecode(text);
    } on FormatException {
      return const NotificationSettings();
    }
    final apps = json is Map ? json['apps'] : null;
    if (apps is! Map) return const NotificationSettings();
    return NotificationSettings({
      for (final MapEntry(:key, :value) in apps.entries)
        if (key is String && value is Map)
          if (_style(value['style']) case final style?) key: style,
    });
  }

  static NotificationStyle? _style(Object? name) => switch (name) {
    'banners' => NotificationStyle.banners,
    'alerts' => NotificationStyle.alerts,
    'none' => NotificationStyle.none,
    _ => null,
  };
}
