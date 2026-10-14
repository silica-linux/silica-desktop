import 'package:flutter_test/flutter_test.dart';
import 'package:silica_shell/system/dbus.dart';
import 'package:silica_shell/system/media.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  test('NowPlaying dalle proprietà MPRIS', () {
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

  test('sceglie il lettore che suona e lo comanda', () async {
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

  test('senza bus: nessun lettore, nessuna eccezione', () async {
    TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
        .setMockMethodCallHandler(DBus.channel, null);
    final players = MediaPlayers();
    await players.refresh();
    expect(players.now, isNull);
  });
}
