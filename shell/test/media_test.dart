import 'package:flutter_test/flutter_test.dart';
import 'package:silica_shell/controlcenter/controlcenter.dart' show mediaTime;
import 'package:silica_shell/system/dbus.dart';
import 'package:silica_shell/system/media.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  test('NowPlaying from the MPRIS properties', () {
    final now = NowPlaying.fromProperties('org.mpris.MediaPlayer2.vlc', {
      'PlaybackStatus': 'Playing',
      'Metadata': {
        'xesam:title': 'Brano',
        'xesam:artist': ['Uno', 'Due'],
        'mpris:artUrl': 'file:///tmp/copertina.png',
      },
      'CanGoNext': true,
      'CanGoPrevious': false,
    });
    expect(now.title, 'Brano');
    expect(now.artist, 'Uno, Due');
    expect(now.playing, isTrue);
    expect(now.canNext, isTrue);
    expect(now.canPrevious, isFalse);
  });

  test('picks the player that is playing and controls it', () async {
    final calls = <String>[];
    TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
        .setMockMethodCallHandler(DBus.channel, (call) async {
          if (call.method == 'subscribe') return 1;
          if (call.method != 'call') return null;
          final args = call.arguments as Map;
          final method = args['method'] as String;
          calls.add('$method ${args['destination']}');
          return switch (method) {
            'ListNames' => [
              [
                'org.freedesktop.DBus',
                'org.mpris.MediaPlayer2.fermo',
                'org.mpris.MediaPlayer2.musica',
              ],
            ],
            'GetAll' => [
              args['destination'] == 'org.mpris.MediaPlayer2.musica'
                  ? {
                      'PlaybackStatus': 'Playing',
                      'Metadata': {'xesam:title': 'Brano'},
                    }
                  : {'PlaybackStatus': 'Stopped', 'Metadata': {}},
            ],
            _ => <Object?>[],
          };
        });
    final players = MediaPlayers();
    await players.refresh();
    expect(players.now?.player, 'org.mpris.MediaPlayer2.musica');
    expect(players.now?.title, 'Brano');
    await players.playPause();
    expect(calls, contains('PlayPause org.mpris.MediaPlayer2.musica'));
  });

  test('without a bus: no player, no exception', () async {
    TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
        .setMockMethodCallHandler(DBus.channel, null);
    final players = MediaPlayers();
    await players.refresh();
    expect(players.now, isNull);
  });

  test('position and length: read, then moved on while playing', () {
    final read = DateTime(2026, 10, 14, 12);
    final now = NowPlaying.fromProperties('org.mpris.MediaPlayer2.x', {
      'PlaybackStatus': 'Playing',
      'Position': 63000000,
      'Metadata': {'mpris:length': 600000000},
    });
    expect(now.length, const Duration(minutes: 10));
    final playing = NowPlaying(
      player: 'p',
      playing: true,
      length: now.length,
      position: now.position,
      readAt: read,
    );
    expect(
      playing.positionAt(read.add(const Duration(seconds: 5))),
      const Duration(seconds: 68),
    );
    // Never past the end.
    expect(playing.positionAt(read.add(const Duration(hours: 1))), now.length);
    final paused = NowPlaying(
      player: 'p',
      position: const Duration(seconds: 63),
      readAt: read,
    );
    expect(
      paused.positionAt(read.add(const Duration(seconds: 5))),
      const Duration(seconds: 63),
    );
    expect(mediaTime(const Duration(seconds: 63)), '01:03');
    expect(mediaTime(const Duration(hours: 1, seconds: 3)), '1:00:03');
  });
}
