// What apps ask of their Dock icon over D-Bus: a badge, a progress bar, attention. Apps on
// Linux send it with the de facto LauncherEntry API (signal `Update` of
// `com.canonical.Unity.LauncherEntry` on the session bus, as GTK, Qt/KDE, Electron and
// Chromium apps do): app URI `application://<desktop id>.desktop` and a property map with
// `count`, `count-visible`, `progress`, `progress-visible`, `urgent`.

import 'dart:async';

import 'package:flutter/foundation.dart';

import '../system/dbus.dart';

/// The state of one app's Dock icon.
@immutable
class LauncherState {
  const LauncherState({this.badge, this.progress, this.urgentSince});

  /// The badge text (SH-DK-29), null without a badge.
  final String? badge;

  /// 0–1, null without a progress bar (SH-DK-30).
  final double? progress;

  /// When the app asked for attention (SH-DK-30), null when it does not.
  final DateTime? urgentSince;

  LauncherState copyWith({
    String? Function()? badge,
    double? Function()? progress,
    DateTime? Function()? urgentSince,
  }) => LauncherState(
    badge: badge == null ? this.badge : badge(),
    progress: progress == null ? this.progress : progress(),
    urgentSince: urgentSince == null ? this.urgentSince : urgentSince(),
  );

  bool get isEmpty => badge == null && progress == null && urgentSince == null;
}

/// The desktop id in a LauncherEntry app URI (`application://firefox.desktop` → `firefox`).
String? launcherAppId(String uri) {
  const scheme = 'application://';
  if (!uri.startsWith(scheme)) return null;
  var id = uri.substring(scheme.length);
  if (id.endsWith('.desktop')) id = id.substring(0, id.length - 8);
  return id.isEmpty ? null : id;
}

/// The badge text of a count: the number, `99+` beyond 99 999 (the reference showed up to
/// five characters without truncating).
String badgeText(int count) => count > 99999 ? '99999+' : '$count';

/// Applies one `Update` to the state of an app.
LauncherState applyLauncherUpdate(
  LauncherState state,
  Map<Object?, Object?> props, {
  DateTime? now,
}) {
  Object? value(String key) {
    final v = props[key];
    // Variants may arrive wrapped as [signature, value].
    if (v is List && v.length == 2 && v.first is String) return v.last;
    return v;
  }

  var next = state;
  final count = value('count');
  final countVisible = value('count-visible');
  if (countVisible is bool || count is num) {
    final visible = countVisible is bool ? countVisible : state.badge != null;
    final n = count is num ? count.toInt() : int.tryParse(state.badge ?? '');
    next = next.copyWith(
      badge: () => visible && n != null && n > 0 ? badgeText(n) : null,
    );
  }
  final progress = value('progress');
  final progressVisible = value('progress-visible');
  if (progressVisible is bool || progress is num) {
    final visible = progressVisible is bool
        ? progressVisible
        : state.progress != null;
    final p = progress is num ? progress.toDouble() : state.progress;
    next = next.copyWith(
      progress: () => visible && p != null ? p.clamp(0.0, 1.0) : null,
    );
  }
  final urgent = value('urgent');
  if (urgent is bool) {
    next = next.copyWith(
      urgentSince: () =>
          urgent ? (state.urgentSince ?? now ?? DateTime.now()) : null,
    );
  }
  return next;
}

/// The LauncherEntry state of every app, from the session bus.
class LauncherEntries {
  LauncherEntries({Stream<DBusSignal>? signals})
    : _signals =
          signals ??
          DBus.instance.signals(
            interface: 'com.canonical.Unity.LauncherEntry',
            member: 'Update',
          );

  final Stream<DBusSignal> _signals;
  final ValueNotifier<Map<String, LauncherState>> value = ValueNotifier(
    const {},
  );
  StreamSubscription<DBusSignal>? _subscription;

  void start() {
    _subscription ??= _signals.listen(
      _update,
      // Outside Silica (tests, another embedder) there is no bus.
      onError: (Object _) {},
    );
  }

  void _update(DBusSignal signal) {
    if (signal.args.length < 2) return;
    final uri = signal.args[0];
    final props = signal.args[1];
    if (uri is! String || props is! Map) return;
    final id = launcherAppId(uri);
    if (id == null) return;
    final map = Map.of(value.value);
    final next = applyLauncherUpdate(map[id] ?? const LauncherState(), props);
    if (next.isEmpty) {
      map.remove(id);
    } else {
      map[id] = next;
    }
    value.value = map;
  }

  /// The app was activated: attention is satisfied (the reference stops bouncing).
  void activated(String id) {
    final state = value.value[id];
    if (state == null || state.urgentSince == null) return;
    final map = Map.of(value.value);
    final next = state.copyWith(urgentSince: () => null);
    if (next.isEmpty) {
      map.remove(id);
    } else {
      map[id] = next;
    }
    value.value = map;
  }

  void dispose() {
    _subscription?.cancel();
    value.dispose();
  }
}
