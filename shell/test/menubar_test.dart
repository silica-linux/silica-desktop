import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:silica_shell/menubar/menubar.dart';
import 'package:silica_shell/system/desktop_entries.dart';
import 'package:silica_shell/system/shell_channel.dart';

void main() {
  test("l'orologio come nella barra: giorno, data, doppio spazio, ora", () {
    expect(clockText(DateTime(2026, 10, 12, 22, 34)), 'Lun 12 ott  22:34');
    expect(clockText(DateTime(2026, 1, 4, 9, 5)), 'Dom 4 gen  09:05');
  });

  testWidgets(
    "il nome dell'app con il focus, in grassetto, dalle voci .desktop",
    (tester) async {
      tester.binding.defaultBinaryMessenger.setMockMethodCallHandler(
        ShellChannel.channel,
        (call) async => switch (call.method) {
          'list' => [
            {'id': 'w1', 'title': 'Casa', 'app_id': 'org.gnome.Nautilus'},
          ],
          'focused' => 'w1',
          _ => null,
        },
      );
      const files = DesktopEntry(
        id: 'org.gnome.Nautilus',
        name: 'File',
        exec: 'nautilus',
      );
      await tester.pumpWidget(
        Directionality(
          textDirection: TextDirection.ltr,
          child: MenuBar(
            entries: const {'org.gnome.Nautilus': files},
            now: DateTime(2026, 10, 12, 22, 34),
          ),
        ),
      );
      await tester.pump();
      await tester.pump();
      final name = tester.widget<Text>(find.text('File').first);
      expect(name.style?.fontWeight, FontWeight.w600);
      expect(find.text('Lun 12 ott  22:34'), findsOneWidget);
    },
  );
}
