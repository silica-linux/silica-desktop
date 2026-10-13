// Le notifiche come le manda silica-daemon (dev.frascella.Silica.Notifications1,
// docs/daemon.md), i gruppi del Centro notifiche e le curve dei banner.

import 'dart:math' as math;

import 'package:flutter/foundation.dart';

/// Una notifica. I campi seguono la struttura `(ussssasysssbbti)` del daemon.
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

  /// Coppie (chiave, etichetta), come nella specifica freedesktop.
  final List<(String, String)> actions;

  /// 0 bassa, 1 normale, 2 critica.
  final int urgency;
  final String desktopEntry;
  final String imagePath;

  /// Solo banner: non resta nel Centro.
  final bool transient;
  final bool resident;
  final DateTime timestamp;

  bool get critical => urgency >= 2;

  /// Le azioni da mostrare come pulsanti: `default` è il clic sulla notifica.
  List<(String, String)> get buttons => [
    for (final action in actions)
      if (action.$1 != 'default') action,
  ];

  bool get hasDefault => actions.any((a) => a.$1 == 'default');

  /// La chiave del gruppo nel Centro: la stessa app.
  String get group => desktopEntry.isNotEmpty ? desktopEntry : appName;

  /// Dalla struttura D-Bus in JSON (una lista di 14 campi); null se non torna.
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

/// Le notifiche di una app nel Centro, dalla più recente.
@immutable
class NotificationGroup {
  const NotificationGroup(this.key, this.items);

  final String key;
  final List<ShellNotification> items;

  ShellNotification get latest => items.first;
}

/// I gruppi del Centro: notifiche non transitorie, per app, i gruppi ordinati dalla
/// notifica più recente.
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

/// Entrata del banner (notifications.md, "Animazione di comparsa"): scorre da destra
/// entrando da fuori schermo, la distanza residua si riduce di ~0,82 a fotogramma
/// (75 Hz), cioè una costante di tempo di ~65 ms, senza superamento.
const double bannerEnterTau = 65;

/// Distanza di partenza: tutto il banner più il margine, fuori dallo schermo.
const double bannerEnterDistance = 344 + 16;

/// Durata dell'entrata: fino a mezzo punto dall'arrivo.
final Duration bannerEnterDuration = Duration(
  milliseconds: (bannerEnterTau * math.log(bannerEnterDistance / 0.5)).ceil(),
);

/// Spostamento a destra del banner `ms` dopo l'inizio dell'entrata.
double bannerEnterOffset(double ms) {
  if (ms <= 0) return bannerEnterDistance;
  final offset = bannerEnterDistance * math.exp(-ms / bannerEnterTau);
  return offset < 0.5 ? 0 : offset;
}

/// Tempo di permanenza di un banner (non misurato: plan/reference/RICHIESTE.md, N-01). Le
/// notifiche critiche restano finché l'utente non le chiude.
const Duration bannerDuration = Duration(seconds: 5);

/// Uscita del banner (non misurata, N-01): verso destra.
const Duration bannerExitDuration = Duration(milliseconds: 250);
