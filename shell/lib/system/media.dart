// The media players on the session bus (MPRIS, `org.mpris.MediaPlayer2.*`): the media
// module of the Control Center shows the one playing (or the first) and controls it with
// PlayPause, Previous and Next. It updates with PropertiesChanged and with players that
// appear or go away (NameOwnerChanged).

import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:flutter/services.dart';

import 'dbus.dart';

const String mprisPrefix = 'org.mpris.MediaPlayer2.';
const String mprisPath = '/org/mpris/MediaPlayer2';
const String mprisPlayer = 'org.mpris.MediaPlayer2.Player';

/// What is playing now.
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
    this.length = Duration.zero,
    this.position = Duration.zero,
    this.readAt,
  });

  /// The name on the bus (`org.mpris.MediaPlayer2.…`).
  final String player;
  final String title;
  final String artist;

  /// The cover (`mpris:artUrl`); only `file://` is drawn.
  final String artUrl;
  final bool playing;
  final bool canPrevious;
  final bool canNext;

  /// The track length (`mpris:length`), zero if the player does not say.
  final Duration length;

  /// The position when it was read (`Position` is not announced as it changes: it is
  /// read with the properties and moved on while playing).
  final Duration position;
  final DateTime? readAt;

  /// The position at `now`: the one read, plus the time since then while playing.
  Duration positionAt(DateTime now) {
    final at = readAt;
    var p = position;
    if (playing && at != null) p += now.difference(at);
    if (length > Duration.zero && p > length) p = length;
    return p < Duration.zero ? Duration.zero : p;
  }

  /// From the properties of `org.mpris.MediaPlayer2.Player`.
  static NowPlaying fromProperties(String player, Map<String, Object?> props) {
    final metadata = props['Metadata'];
    final meta = metadata is Map ? metadata : const {};
    String text(Object? value) => switch (value) {
      final String s => s,
      final List<Object?> l => l.whereType<String>().join(', '),
      _ => '',
    };
    int micros(Object? value) => value is int ? value : 0;
    return NowPlaying(
      player: player,
      length: Duration(microseconds: micros(meta['mpris:length'])),
      position: Duration(microseconds: micros(props['Position'])),
      readAt: DateTime.now(),
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

  /// The player shown: the one playing, otherwise the first; null if there are none.
  NowPlaying? get now => _now;

  /// Starts following the players (once).
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

  /// Re-reads the players and their properties.
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
          // A player that does not answer: skipped.
        }
      }
      _set(players.where((p) => p.playing).firstOrNull ?? players.firstOrNull);
    } on PlatformException {
      _set(null);
    } on MissingPluginException {
      // Outside Silica: no bus.
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
        old?.canNext == value?.canNext &&
        old?.length == value?.length &&
        (old?.positionAt(DateTime.now()) ?? Duration.zero) -
                (value?.positionAt(DateTime.now()) ?? Duration.zero) <
            const Duration(seconds: 2) &&
        (value?.positionAt(DateTime.now()) ?? Duration.zero) -
                (old?.positionAt(DateTime.now()) ?? Duration.zero) <
            const Duration(seconds: 2)) {
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
      // The player went away: NameOwnerChanged will say so.
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
