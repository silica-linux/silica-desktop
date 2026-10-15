// The arrangement popover follows the appearance of the window it was opened from
// (`windowMenuAppearance` right before `windowMenu`, silica-shell-v1 version 8).

import 'dart:ui' as ui;

import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:silica_shell/system/shell_channel.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  Future<void> send(String method, Object? args) =>
      TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
          .handlePlatformMessage(
            'silica/shell',
            const JSONMethodCodec().encodeMethodCall(MethodCall(method, args)),
            (_) {},
          );

  Future<ui.Brightness?> open(String id, String? announced) async {
    final menu = ShellChannel.instance.windowMenus.first;
    if (announced != null) {
      await send('windowMenuAppearance', {'id': id, 'appearance': announced});
    }
    await send('windowMenu', {'id': 'w1', 'x': 10, 'y': 20});
    return (await menu).brightness;
  }

  test('a dark window opens the popover in dark', () async {
    expect(await open('w1', 'dark'), ui.Brightness.dark);
  });

  test('a light window opens the popover in light', () async {
    expect(await open('w1', 'light'), ui.Brightness.light);
  });

  test('a window that follows the system leaves the brightness null', () async {
    expect(await open('w1', 'system'), isNull);
  });

  test('an appearance for another window, or none, is ignored', () async {
    expect(await open('other', 'dark'), isNull);
    expect(await open('w1', null), isNull);
  });

  test('the appearance is used once', () async {
    expect(await open('w1', 'dark'), ui.Brightness.dark);
    expect(await open('w1', null), isNull);
  });
}
