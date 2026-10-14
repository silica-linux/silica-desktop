import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:silica_shell/system/dbus.dart';
import 'package:silica_shell/system/sounds.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  test('a sound is a PlaySound call to the daemon', () async {
    final calls = <MethodCall>[];
    TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
        .setMockMethodCallHandler(DBus.channel, (call) async {
          calls.add(call);
          return const <Object?>[];
        });
    addTearDown(
      () => TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
          .setMockMethodCallHandler(DBus.channel, null),
    );
    await playUiSound(UiSound.trashEmpty);
    expect(calls.single.method, 'call');
    expect(calls.single.arguments, {
      'bus': 'session',
      'destination': 'dev.frascella.Silica',
      'path': '/dev/frascella/Silica/Sound1',
      'interface': 'dev.frascella.Silica.Sound1',
      'method': 'PlaySound',
      'signature': 's',
      'args': ['trash-empty'],
    });
  });

  test('without the daemon, silence and no error', () async {
    TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
        .setMockMethodCallHandler(DBus.channel, (call) async {
          throw PlatformException(
            code: 'org.freedesktop.DBus.Error.ServiceUnknown',
          );
        });
    addTearDown(
      () => TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
          .setMockMethodCallHandler(DBus.channel, null),
    );
    await playUiSound(UiSound.trashDrag);
    TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
        .setMockMethodCallHandler(DBus.channel, null);
    await playUiSound(UiSound.fileTransfer);
  });
}
