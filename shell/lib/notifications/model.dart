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

/// Entrata del banner (notifications.md, "Ciclo di vita del banner"): scorre da destra
/// entrando da fuori schermo; la distanza residua si riduce di ~0,83 a fotogramma (75 Hz),
/// costante di tempo ~71 ms, arrivo entro 1 pt in ~400 ms, senza superamento.
const double bannerEnterTau = 71;

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

/// Mentre entra il banner compare in dissolvenza (~210 ms) ed è sfocato, a fuoco in ~215 ms.
const double bannerFadeInMs = 210;
const double bannerFocusMs = 215;

/// Sfocatura massima (σ in pt) del banner che entra o esce: il contorno del contenuto
/// sparisce quasi del tutto nel primo fotogramma (gradiente massimo 4 su 118).
const double bannerBlurSigma = 8;

/// Opacità e sfocatura dell'entrata `ms` dopo l'inizio.
double bannerEnterOpacity(double ms) => (ms / bannerFadeInMs).clamp(0.0, 1.0);
double bannerEnterBlur(double ms) =>
    bannerBlurSigma * (1 - (ms / bannerFocusMs).clamp(0.0, 1.0));

/// Dalla comparsa all'inizio dell'uscita: ~5,0 s. Le notifiche critiche restano finché
/// l'utente non le chiude.
const Duration bannerDuration = Duration(seconds: 5);

/// Con il puntatore sopra il banner non scade; dopo che il puntatore esce, ~1,1 s.
const Duration bannerLinger = Duration(milliseconds: 1100);

/// Uscita: verso destra, ~180–190 ms, con dissolvenza fino a metà e sfocatura crescente.
const Duration bannerExitDuration = Duration(milliseconds: 190);

/// Spostamenti misurati dell'uscita a passi di ~13,3 ms (bordo sinistro − 2200, pt).
const List<double> bannerExitSteps = [
  0,
  37,
  66,
  104,
  141,
  175,
  207,
  234,
  264,
  287,
  305,
  321,
  335,
  346,
  356,
];

/// Spostamento a destra `ms` dopo l'inizio dell'uscita (interpolazione delle misure).
double bannerExitOffset(double ms) {
  final step = ms / (1000 / 75);
  if (step <= 0) return 0;
  final i = step.floor();
  if (i >= bannerExitSteps.length - 1) return bannerEnterDistance;
  return bannerExitSteps[i] +
      (bannerExitSteps[i + 1] - bannerExitSteps[i]) * (step - i);
}

double bannerExitOpacity(double ms) =>
    1 - 0.5 * (ms / bannerExitDuration.inMilliseconds).clamp(0.0, 1.0);
double bannerExitBlur(double ms) =>
    bannerBlurSigma * (ms / bannerExitDuration.inMilliseconds).clamp(0.0, 1.0);

/// Apertura del Centro notifiche: scorre da destra, quasi tutto in ~35 ms e poi una coda
/// lunga fino a ~235 ms (bordo dei contenuti 2545 → 2198 → 2158): somma di due
/// esponenziali, una rapida e una lenta.
const double centerDistance = 387;
const Duration centerOpenDuration = Duration(milliseconds: 235);

double centerOpenOffset(double ms) {
  if (ms <= 0) return centerDistance;
  final x = 347 * math.exp(-ms / 12) + 40 * math.exp(-ms / 70);
  return ms >= centerOpenDuration.inMilliseconds || x < 0.5 ? 0 : x;
}

/// Chiusura (~300 ms, uguale con Esc, clic sull'orologio o clic fuori): un leggero
/// spostamento lento, poi un'uscita rapida e una coda.
const Duration centerCloseDuration = Duration(milliseconds: 300);
const List<(double, double)> centerCloseSteps = [
  (0, 0),
  (60, 10),
  (120, 37),
  (135, 104),
  (150, 357),
  (200, 380),
  (300, centerDistance),
];

double centerCloseOffset(double ms) {
  if (ms <= 0) return 0;
  for (var i = 1; i < centerCloseSteps.length; i++) {
    final (t1, x1) = centerCloseSteps[i];
    if (ms <= t1) {
      final (t0, x0) = centerCloseSteps[i - 1];
      return x0 + (x1 - x0) * (ms - t0) / (t1 - t0);
    }
  }
  return centerDistance;
}
