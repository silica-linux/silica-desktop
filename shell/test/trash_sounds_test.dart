import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:silica_shell/dock/trash_alert.dart';

void main() {
  late Directory home;
  setUp(() => home = Directory.systemTemp.createTempSync('trash_sounds'));
  tearDown(() => home.deleteSync(recursive: true));

  test('a drop on the Trash sounds once, after the items are in', () async {
    final file = File('${home.path}/a.txt')..writeAsStringSync('a');
    final played = <String>[];
    var existedWhenPlayed = true;
    await trashDropped(
      [file.path],
      env: {'HOME': home.path},
      play: (name) async {
        played.add(name);
        existedWhenPlayed = file.existsSync();
      },
    );
    expect(played, ['trash-move']);
    expect(existedWhenPlayed, isFalse);
    expect(
      File('${home.path}/.local/share/Trash/files/a.txt').existsSync(),
      isTrue,
    );
  });

  test('nothing dropped, no sound', () async {
    final played = <String>[];
    await trashDropped(
      const [],
      env: {'HOME': home.path},
      play: (name) async => played.add(name),
    );
    expect(played, isEmpty);
  });

  test('Empty Trash sounds after a delay, once', () async {
    final played = <String>[];
    final started = DateTime.now();
    await soundTrashEmptied(
      delay: const Duration(milliseconds: 50),
      play: (name) async => played.add(name),
    );
    expect(played, ['trash-empty']);
    expect(
      DateTime.now().difference(started),
      greaterThanOrEqualTo(const Duration(milliseconds: 45)),
    );
    expect(trashEmptiedSoundDelay, const Duration(milliseconds: 200));
  });
}
