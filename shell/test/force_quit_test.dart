import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:silica_shell/forcequit/force_quit.dart';
import 'package:silica_shell/system/desktop_entries.dart';
import 'package:silica_shell/system/shell_channel.dart';
import 'package:silica_ui/silica_ui.dart';

void main() {
  testWidgets('⌘⌥Esc: the open apps, the frontmost one chosen, Quit', (
    tester,
  ) async {
    tester.view.physicalSize = const Size(1280, 800);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.reset);
    final executed = <Object?>[];
    tester.binding.defaultBinaryMessenger
      ..setMockMethodCallHandler(ShellChannel.channel, (call) async {
        return switch (call.method) {
          'list' => [
            {'id': 'w1', 'title': 'Casa', 'app_id': 'org.gnome.Nautilus'},
            {'id': 'w2', 'title': 'Nota', 'app_id': 'org.gnome.TextEditor'},
            {'id': 'w3', 'title': 'Altra', 'app_id': 'org.gnome.TextEditor'},
          ],
          'focused' => 'w2',
          'execute' => () {
            executed.add(call.arguments);
            return true;
          }(),
          _ => null,
        };
      })
      ..setMockMethodCallHandler(
        const MethodChannel('silica/layer', JSONMethodCodec()),
        (call) async => true,
      );
    await ShellChannel.instance.refresh();
    const entries = {
      'org.gnome.Nautilus': DesktopEntry(
        id: 'org.gnome.Nautilus',
        name: 'File',
        exec: 'nautilus',
      ),
      'org.gnome.TextEditor': DesktopEntry(
        id: 'org.gnome.TextEditor',
        name: 'Editor di testo',
        exec: 'gnome-text-editor',
      ),
    };
    await tester.pumpWidget(
      const Directionality(
        textDirection: TextDirection.ltr,
        child: ForceQuitLayer(entries: entries),
      ),
    );
    await tester.binding.defaultBinaryMessenger.handlePlatformMessage(
      'silica/shell',
      const JSONMethodCodec().encodeMethodCall(
        MethodCall('command', 'app.force_quit_panel'),
      ),
      (_) {},
    );
    await tester.pump();
    final panel = tester.getRect(find.byKey(const ValueKey('force quit')));
    expect(panel.size, forceQuitSize);
    expect(panel.center, const Offset(640, 400));
    // The list: 28 pt rows inside the padding of its surface.
    final list = tester.getRect(find.byKey(const ValueKey('force quit list')));
    expect(list.left - panel.left, 20);
    // One row per app, by name.
    expect(find.text('Editor di testo'), findsOneWidget);
    expect(find.text('File'), findsOneWidget);
    expect(
      tester.getTopLeft(find.text('Editor di testo')).dy,
      lessThan(tester.getTopLeft(find.text('File')).dy),
    );
    await tester.tap(find.byKey(const ValueKey('force quit button')));
    await tester.pump();
    expect(executed.single, {
      'id': 'app.force_quit',
      'params': {'app_id': 'org.gnome.TextEditor'},
    });
    expect(find.byKey(const ValueKey('force quit')), findsNothing);
  });

  testWidgets('keys: arrows move the choice, Return quits it, Esc closes', (
    tester,
  ) async {
    tester.view.physicalSize = const Size(1280, 800);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.reset);
    final executed = <Object?>[];
    tester.binding.defaultBinaryMessenger
      ..setMockMethodCallHandler(ShellChannel.channel, (call) async {
        return switch (call.method) {
          'list' => [
            {'id': 'w1', 'title': 'A', 'app_id': 'a'},
            {'id': 'w2', 'title': 'B', 'app_id': 'b'},
            {'id': 'w3', 'title': 'C', 'app_id': 'c'},
          ],
          'focused' => 'w1',
          'execute' => () {
            executed.add(call.arguments);
            return true;
          }(),
          _ => null,
        };
      })
      ..setMockMethodCallHandler(
        const MethodChannel('silica/layer', JSONMethodCodec()),
        (call) async => true,
      );
    await ShellChannel.instance.refresh();
    await tester.pumpWidget(
      const Directionality(
        textDirection: TextDirection.ltr,
        child: ForceQuitLayer(entries: {}),
      ),
    );
    Future<void> open() async {
      await tester.binding.defaultBinaryMessenger.handlePlatformMessage(
        'silica/shell',
        const JSONMethodCodec().encodeMethodCall(
          MethodCall('command', 'app.force_quit_panel'),
        ),
        (_) {},
      );
      await tester.pump();
      await tester.pump();
    }

    await open();
    // The frontmost app (A) is chosen: its row is the Accent Soft capsule.
    Color? rowFill(String id) {
      final box = tester.widget<DecoratedBox>(
        find
            .descendant(
              of: find.byKey(ValueKey('app $id')),
              matching: find.byType(DecoratedBox),
            )
            .first,
      );
      return (box.decoration as AmalfiShapeDecoration).color;
    }

    expect(rowFill('a'), AmalfiColors.light.accentAmalfiSoft);
    expect(rowFill('b'), isNull);
    await tester.sendKeyEvent(LogicalKeyboardKey.arrowDown);
    await tester.pump();
    expect(rowFill('a'), isNull);
    expect(rowFill('b'), AmalfiColors.light.accentAmalfiSoft);
    await tester.sendKeyEvent(LogicalKeyboardKey.enter);
    await tester.pump();
    expect(executed.single, {
      'id': 'app.force_quit',
      'params': {'app_id': 'b'},
    });
    expect(find.byKey(const ValueKey('force quit')), findsNothing);
    // Esc closes it without quitting anything.
    await open();
    await tester.sendKeyEvent(LogicalKeyboardKey.escape);
    await tester.pump();
    expect(find.byKey(const ValueKey('force quit')), findsNothing);
    expect(executed, hasLength(1));
  });

  testWidgets('with no app chosen the Force Quit button is disabled', (
    tester,
  ) async {
    tester.view.physicalSize = const Size(1280, 800);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.reset);
    tester.binding.defaultBinaryMessenger
      ..setMockMethodCallHandler(ShellChannel.channel, (call) async {
        return switch (call.method) {
          'list' => [
            {'id': 'w1', 'title': 'A', 'app_id': 'a'},
          ],
          'focused' => 'none',
          _ => null,
        };
      })
      ..setMockMethodCallHandler(
        const MethodChannel('silica/layer', JSONMethodCodec()),
        (call) async => true,
      );
    await ShellChannel.instance.refresh();
    await tester.pumpWidget(
      const Directionality(
        textDirection: TextDirection.ltr,
        child: ForceQuitLayer(entries: {}),
      ),
    );
    await tester.binding.defaultBinaryMessenger.handlePlatformMessage(
      'silica/shell',
      const JSONMethodCodec().encodeMethodCall(
        MethodCall('command', 'app.force_quit_panel'),
      ),
      (_) {},
    );
    await tester.pump();
    final button = tester.widget<AmalfiPushButton>(
      find.byKey(const ValueKey('force quit button')),
    );
    expect(button.onPressed, isNull);
    expect(button.variant, AmalfiPushButtonVariant.primary);
  });
}
