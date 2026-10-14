import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:silica_shell/system/dbus.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();
  final messenger =
      TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger;

  tearDown(() => messenger.setMockMethodCallHandler(DBus.channel, null));

  test(
    'call sends the signature and the arguments and returns the reply list',
    () async {
      final calls = <MethodCall>[];
      messenger.setMockMethodCallHandler(DBus.channel, (call) async {
        calls.add(call);
        return [
          {'Percentage': 80.0},
        ];
      });
      final props = await DBus.instance.getAll(
        bus: DBusBus.system,
        destination: 'org.freedesktop.UPower',
        path: '/org/freedesktop/UPower/devices/DisplayDevice',
        interface: 'org.freedesktop.UPower.Device',
      );
      expect(props['Percentage'], 80.0);
      expect(calls.single.method, 'call');
      final args = calls.single.arguments as Map;
      expect(args['bus'], 'system');
      expect(args['method'], 'GetAll');
      expect(args['signature'], 's');
      expect(args['args'], ['org.freedesktop.UPower.Device']);
    },
  );

  test('D-Bus errors arrive with their name', () async {
    messenger.setMockMethodCallHandler(DBus.channel, (call) async {
      throw PlatformException(
        code: 'org.freedesktop.DBus.Error.ServiceUnknown',
        message: 'assente',
      );
    });
    expect(
      DBus.instance.call(destination: 'x.y', path: '/', method: 'M'),
      throwsA(
        isA<PlatformException>().having(
          (e) => e.code,
          'code',
          'org.freedesktop.DBus.Error.ServiceUnknown',
        ),
      ),
    );
  });

  test(
    'signals reach the right subscription while it is listened to',
    () async {
      final calls = <MethodCall>[];
      messenger.setMockMethodCallHandler(DBus.channel, (call) async {
        calls.add(call);
        return call.method == 'subscribe' ? 7 : true;
      });
      final received = <DBusSignal>[];
      final subscription = DBus.instance
          .signals(interface: 'dev.frascella.Silica.Notifications1')
          .listen(received.add);
      await pumpEventQueue();
      expect(calls.single.method, 'subscribe');

      Future<void> send(int id) => messenger.handlePlatformMessage(
        'silica/dbus',
        const JSONMethodCodec().encodeMethodCall(
          MethodCall('signal', {
            'id': id,
            'sender': ':1.4',
            'path': '/dev/frascella/Silica/Notifications1',
            'interface': 'dev.frascella.Silica.Notifications1',
            'member': 'Removed',
            'args': [3, 2],
          }),
        ),
        (_) {},
      );
      await send(7);
      await send(8);
      expect(received, hasLength(1));
      expect(received.single.member, 'Removed');
      expect(received.single.args, [3, 2]);

      await subscription.cancel();
      expect(calls.last.method, 'unsubscribe');
      expect((calls.last.arguments as Map)['id'], 7);
      await send(7);
      expect(received, hasLength(1));
    },
  );
}
