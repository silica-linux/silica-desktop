import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:silica_shell/forcequit/force_quit.dart';
import 'package:silica_shell/system/desktop_entries.dart';
import 'package:silica_shell/system/shell_channel.dart';

void main() {
  testWidgets('⌘⌥Esc: le app aperte, quella in primo piano scelta, Termina', (
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
    final panel = tester.getRect(find.byKey(const ValueKey('uscita forzata')));
    expect(panel.size, forceQuitSize);
    expect(panel.center, const Offset(640, 400));
    // Una riga per app, per nome.
    expect(find.text('Editor di testo'), findsOneWidget);
    expect(find.text('File'), findsOneWidget);
    expect(
      tester.getTopLeft(find.text('Editor di testo')).dy,
      lessThan(tester.getTopLeft(find.text('File')).dy),
    );
    await tester.tap(find.byKey(const ValueKey('termina')));
    await tester.pump();
    expect(executed.single, {
      'id': 'app.force_quit',
      'params': {'app_id': 'org.gnome.TextEditor'},
    });
    expect(find.byKey(const ValueKey('uscita forzata')), findsNothing);
  });
}
