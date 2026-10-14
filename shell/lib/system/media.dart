// I lettori multimediali sul bus di sessione (MPRIS, `org.mpris.MediaPlayer2.*`): il modulo
// multimediale del Centro di controllo mostra quello che suona (o il primo) e lo comanda con
// PlayPause, Previous e Next. Si aggiorna con PropertiesChanged e con i lettori che
// compaiono o se ne vanno (NameOwnerChanged).

import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:flutter/services.dart';

import 'dbus.dart';

const String mprisPrefix = 'org.mpris.MediaPlayer2.';
const String mprisPath = '/org/mpris/MediaPlayer2';
const String mprisPlayer = 'org.mpris.MediaPlayer2.Player';

/// Quello che suona ora.
@immutable
class NowPlaying {
  const NowPlaying({
    required this.player,
    this.title = '',
    this.artist = '',
    this.artUrl = '',
    this.playing = false,
    this.canPrevious = false,
    this.canNext = false,
  });

  /// Il nome sul bus (`org.mpris.MediaPlayer2.…`).
  final String player;
  final String title;
  final String artist;

  /// La copertina (`mpris:artUrl`); solo `file://` si disegna.
  final String artUrl;
  final bool playing;
  final bool canPrevious;
  final bool canNext;

  /// Dalle proprietà di `org.mpris.MediaPlayer2.Player`.
  static NowPlaying fromProperties(String player, Map<String, Object?> props) {
    final metadata = props['Metadata'];
    final meta = metadata is Map ? metadata : const {};
    String text(Object? value) => switch (value) {
      final String s => s,
      final List<Object?> l => l.whereType<String>().join(', '),
      _ => '',
    };
    return NowPlaying(
      player: player,
      title: text(meta['xesam:title']),
      artist: text(meta['xesam:artist']),
      artUrl: text(meta['mpris:artUrl']),
      playing: props['PlaybackStatus'] == 'Playing',
      canPrevious: props['CanGoPrevious'] == true,
      canNext: props['CanGoNext'] == true,
    );
  }
}

class MediaPlayers extends ChangeNotifier {
  MediaPlayers({DBus? dbus}) : _dbus = dbus ?? DBus.instance;

  static final MediaPlayers instance = MediaPlayers();

  final DBus _dbus;
  NowPlaying? _now;
  final List<StreamSubscription<DBusSignal>> _signals = [];
  bool _started = false;

  /// Il lettore mostrato: quello che suona, altrimenti il primo; null se non ce ne sono.
  NowPlaying? get now => _now;

  /// Comincia a seguire i lettori (una volta).
  void start() {
    if (_started) return;
    _started = true;
    _signals
      ..add(
        _dbus
            .signals(
              path: mprisPath,
              interface: 'org.freedesktop.DBus.Properties',
              member: 'PropertiesChanged',
            )
            .listen((_) => refresh(), onError: (_) {}),
      )
      ..add(
        _dbus
            .signals(
              sender: 'org.freedesktop.DBus',
              interface: 'org.freedesktop.DBus',
              member: 'NameOwnerChanged',
            )
            .listen((signal) {
              final name = signal.args.isEmpty ? null : signal.args.first;
              if (name is String && name.startsWith(mprisPrefix)) refresh();
            }, onError: (_) {}),
      );
    refresh();
  }

  /// Rilegge i lettori e le loro proprietà.
  Future<void> refresh() async {
    try {
      final reply = await _dbus.call(
        destination: 'org.freedesktop.DBus',
        path: '/org/freedesktop/DBus',
        interface: 'org.freedesktop.DBus',
        method: 'ListNames',
      );
      final names = [
        for (final n in reply.isEmpty ? const [] : reply.first as List? ?? [])
          if (n is String && n.startsWith(mprisPrefix)) n,
      ];
      final players = <NowPlaying>[];
      for (final name in names) {
        try {
          final props = await _dbus.getAll(
            destination: name,
            path: mprisPath,
            interface: mprisPlayer,
          );
          players.add(NowPlaying.fromProperties(name, props));
        } on PlatformException {
          // Un lettore che non risponde: si salta.
        }
      }
      _set(players.where((p) => p.playing).firstOrNull ?? players.firstOrNull);
    } on PlatformException {
      _set(null);
    } on MissingPluginException {
      // Fuori da Silica: nessun bus.
      _set(null);
    }
  }

  void _set(NowPlaying? value) {
    final old = _now;
    if (old?.player == value?.player &&
        old?.title == value?.title &&
        old?.artist == value?.artist &&
        old?.artUrl == value?.artUrl &&
        old?.playing == value?.playing &&
        old?.canPrevious == value?.canPrevious &&
        old?.canNext == value?.canNext) {
      return;
    }
    _now = value;
    notifyListeners();
  }

  Future<void> _command(String method) async {
    final player = _now?.player;
    if (player == null) return;
    try {
      await _dbus.call(
        destination: player,
        path: mprisPath,
        interface: mprisPlayer,
        method: method,
      );
    } on PlatformException {
      // Il lettore se n'è andato: lo dirà NameOwnerChanged.
    }
    await refresh();
  }

  Future<void> playPause() => _command('PlayPause');
  Future<void> previous() => _command('Previous');
  Future<void> next() => _command('Next');

  @override
  void dispose() {
    for (final s in _signals) {
      s.cancel();
    }
    super.dispose();
  }
}
