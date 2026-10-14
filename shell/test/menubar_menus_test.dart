import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:silica_shell/l10n/l10n.dart';
import 'package:silica_shell/menubar/menubar.dart';
import 'package:silica_shell/menubar/menus.dart';
import 'package:silica_ui/silica_ui.dart' show MenuAction;

void main() {
  test('no dead bar menus: Edit is empty until apps publish menus', () {
    expect(barMenu('edit', 'Mail', (_) {}), isEmpty);
  });

  test('Help › Silica Help opens the documentation', () {
    String? opened;
    final help = barMenu('help', 'Mail', (_) {}, openUrl: (u) => opened = u);
    final item = help.whereType<MenuAction>().single;
    expect(item.label, englishTexts.silicaHelp);
    item.onSelected!();
    expect(opened, silicaHelpUrl);
    // Without a way to open links the item is disabled, never dead.
    final off = barMenu('help', 'Mail', (_) {}).whereType<MenuAction>().single;
    expect(off.enabled, isFalse);
  });

  testWidgets('the bar shows no title for an empty menu', (tester) async {
    await tester.pumpWidget(
      Directionality(
        textDirection: TextDirection.ltr,
        child: MenuBar(now: DateTime(2026, 10, 13, 9)),
      ),
    );
    expect(find.text(englishTexts.menuEdit), findsNothing);
    expect(find.text(englishTexts.menuHelp), findsOneWidget);
    expect(find.text(englishTexts.menuWindow), findsOneWidget);
  });
}
