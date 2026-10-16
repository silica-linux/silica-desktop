// Notifications as silica-daemon sends them (dev.frascella.Silica.Notifications1,
// docs/daemon.md), the Notification Centre groups and the Amalfi motion of banners and panel.

import 'dart:math' as math;

import 'package:flutter/foundation.dart';
import 'package:flutter/physics.dart';
import 'package:silica_ui/silica_ui.dart';

/// The action key of the reply field.
const String replyAction = 'inline-reply';

/// A notification. The fields follow the daemon's `(ussssasysssbbti)` structure.
@immutable
class ShellNotification {
  const ShellNotification({
    required this.id,
    required this.appName,
    this.appIcon = '',
    required this.summary,
    this.body = '',
    this.actions = const [],
    this.urgency = 1,
    this.desktopEntry = '',
    this.imagePath = '',
    this.transient = false,
    this.resident = false,
    required this.timestamp,
  });

  final int id;
  final String appName;
  final String appIcon;
  final String summary;
  final String body;

  /// (key, label) pairs, as in the freedesktop specification.
  final List<(String, String)> actions;

  /// 0 low, 1 normal, 2 critical.
  final int urgency;
  final String desktopEntry;
  final String imagePath;

  /// Banner only: it does not stay in the Center.
  final bool transient;
  final bool resident;
  final DateTime timestamp;

  bool get critical => urgency >= 2;

  /// The actions to show as buttons: `default` is the click on the notification, and
  /// `inline-reply` is the reply field.
  List<(String, String)> get buttons => [
    for (final action in actions)
      if (action.$1 != 'default' && action.$1 != replyAction) action,
  ];

  /// The app takes a reply typed in the notification (KDE's inline-reply extension).
  bool get canReply => actions.any((a) => a.$1 == replyAction);

  /// The label of the reply action: the field's placeholder.
  String get replyLabel =>
      actions.firstWhere((a) => a.$1 == replyAction, orElse: () => ('', '')).$2;

  bool get hasDefault => actions.any((a) => a.$1 == 'default');

  /// The group key in the Center: the same app.
  String get group => desktopEntry.isNotEmpty ? desktopEntry : appName;

  /// From the D-Bus structure in JSON (a list of 14 fields); null if it does not fit.
  static ShellNotification? fromDBus(Object? json) {
    if (json is! List || json.length < 14) return null;
    final id = json[0];
    if (id is! int) return null;
    String text(int i) => json[i] is String ? json[i] as String : '';
    final flat = json[5] is List
        ? (json[5] as List).whereType<String>()
        : const <String>[];
    final pairs = flat.toList();
    final timestamp = json[12] is int ? json[12] as int : 0;
    return ShellNotification(
      id: id,
      appName: text(1),
      appIcon: text(2),
      summary: text(3),
      body: text(4),
      actions: [
        for (var i = 0; i + 1 < pairs.length; i += 2) (pairs[i], pairs[i + 1]),
      ],
      urgency: json[6] is int ? json[6] as int : 1,
      desktopEntry: text(8),
      imagePath: text(9),
      transient: json[10] == true,
      resident: json[11] == true,
      timestamp: DateTime.fromMillisecondsSinceEpoch(timestamp),
    );
  }
}

/// The notifications of one app in the Center, from the most recent.
@immutable
class NotificationGroup {
  const NotificationGroup(this.key, this.items);

  final String key;
  final List<ShellNotification> items;

  ShellNotification get latest => items.first;
}

/// The Center groups: non-transient notifications, by app, groups ordered by the most
/// recent notification.
List<NotificationGroup> groupNotifications(Iterable<ShellNotification> all) {
  final sorted =
      [
        for (final n in all)
          if (!n.transient) n,
      ]..sort(
        (a, b) => b.timestamp.compareTo(a.timestamp) != 0
            ? b.timestamp.compareTo(a.timestamp)
            : b.id.compareTo(a.id),
      );
  final groups = <String, List<ShellNotification>>{};
  for (final n in sorted) {
    groups.putIfAbsent(n.group, () => []).add(n);
  }
  return [for (final e in groups.entries) NotificationGroup(e.key, e.value)];
}

/// Banner entrance (motion.md, Notifications: "slide from the trailing edge + fade,
/// `spring.panel`"): the banner starts off screen, the whole banner plus the margin to the
/// right of its place, and settles with `spring.panel` (it overshoots by ~1-2 %, a few points).
const double bannerEnterDistance = 360 + 16;

/// How long the entrance is simulated: `spring.panel` (response 0.35 s, zeta 0.82) is within
/// 0.1 % of its place in ~0.45 s.
const Duration bannerEnterDuration = Duration(milliseconds: 450);

final SpringSimulation _slide = SpringSimulation(AmalfiSprings.panel, 0, 1, 0);

/// How far (0-1) the spring has taken a surface from its start to its place [ms] after it
/// started (1 once [duration] has passed).
double springProgress(double ms, Duration duration) =>
    ms >= duration.inMilliseconds ? 1 : _slide.x(math.max(0, ms) / 1000);

/// Rightward offset of the banner `ms` after the start of the entrance.
double bannerEnterOffset(double ms) =>
    bannerEnterDistance * (1 - springProgress(ms, bannerEnterDuration));

/// Opacity of the entrance `ms` after the start: `fade.quick` (the opacity reaches 1 long
/// before the geometry settles, motion.md rule 1).
double bannerEnterOpacity(double ms) => AmalfiFades.quick.curve.transform(
  (ms / AmalfiFades.quick.duration.inMilliseconds).clamp(0.0, 1.0),
);

/// From appearing to the start of the exit: ~5.0 s. Critical notifications stay until the
/// user closes them.
const Duration bannerDuration = Duration(seconds: 5);

/// With the pointer over it the banner does not expire; after the pointer exits, ~1.1 s.
const Duration bannerLinger = Duration(milliseconds: 1100);

/// Exit: `fade.standard` (250 ms, ease-in-out), nothing else moves.
final Duration bannerExitDuration = AmalfiFades.standard.duration;

/// Opacity of the exit `ms` after its start.
double bannerExitOpacity(double ms) =>
    1 -
    AmalfiFades.standard.curve.transform(
      (ms / AmalfiFades.standard.duration.inMilliseconds).clamp(0.0, 1.0),
    );

/// Notification Centre opening: like a banner, it slides in from the trailing edge and fades
/// in with `spring.panel`. Closing is `fade.instant` (a transient surface closing; the
/// reference closing curve is not carried over: assumption, motion.md has no row for it).
const double centerDistance = 380 + 16;
const Duration centerOpenDuration = Duration(milliseconds: 450);
final Duration centerCloseDuration = AmalfiFades.instant.duration;

double centerOpenOffset(double ms) =>
    centerDistance * (1 - springProgress(ms, centerOpenDuration));

double centerOpenOpacity(double ms) => bannerEnterOpacity(ms);

double centerCloseOpacity(double ms) =>
    1 -
    AmalfiFades.instant.curve.transform(
      (ms / AmalfiFades.instant.duration.inMilliseconds).clamp(0.0, 1.0),
    );
