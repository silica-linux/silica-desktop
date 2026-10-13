import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:silica_shell/switcher/switcher.dart';
import 'package:silica_shell/system/desktop_entries.dart';
import 'package:silica_shell/system/shell_channel.dart';

const entries = {
  'term': DesktopEntry(id: 'term', name: 'Terminale', exec: 'terminale'),
  'files': DesktopEntry(id: 'files', name: 'File', exec: 'files'),
};

Toplevel window(String id, String app, [String title = '']) =>
    Toplevel(id: id, title: title, appId: app);

void main() {
  test('le app seguono l\'ordine d\'uso delle finestre', () {
    final model = SwitcherModel(entries: entries)
      ..toplevels([
        window('1', 'term'),
        window('2', 'files'),
        window('3', 'term'),
        window('4', '', 'Senza app'),
      ])
      ..focused('2')
      ..focused('3')
      ..focused('4');
    final apps = model.recentApps();
    expect([for (final a in apps) a.name], ['Senza app', 'Terminale', 'File']);
    // Le finestre dell'app dalla più recente.
    expect(apps[1].windows, ['3', '1']);
  });

  test('⌘Tab sceglie l\'app usata prima; ⌘ rilasciato la attiva', () {
    final model = SwitcherModel(entries: entries)
      ..toplevels([window('1', 'term'), window('2', 'files')])
      ..focused('1')
      ..focused('2');
    expect(model.visible, isFalse);
    model.next();
    expect(model.visible, isTrue);
    expect(model.apps![model.selected].name, 'Terminale');
    model.next();
    expect(model.selected, 0);
    model.previous();
    expect(model.commit(), '1');
    expect(model.visible, isFalse);
    // ⇧⌘Tab apre sull'ultima; Esc annulla.
    model.previous();
    expect(model.selected, 1);
    model.cancel();
    expect(model.commit(), isNull);
    // Le finestre chiuse escono dall'ordine.
    model.toplevels([window('2', 'files')]);
    expect(
      [for (final a in model.recentApps()) a.windows],
      [
        ['2'],
      ],
    );
  });

  testWidgets(
    'quattro app: pannello 580 × 176 centrato, riquadro 120 attorno alla scelta',
    (tester) async {
      tester.view.physicalSize = const Size(2560, 1440);
      tester.view.devicePixelRatio = 1;
      addTearDown(tester.view.reset);
      final apps = [
        for (var i = 0; i < 4; i++)
          SwitcherApp(key: '$i', name: 'App $i', windows: ['$i']),
      ];
      await tester.pumpWidget(
        Directionality(
          textDirection: TextDirection.ltr,
          child: SwitcherPanel(apps: apps, selected: 1),
        ),
      );
      expect(panelWidth(4), 580);
      final panel = tester.getRect(find.byType(Positioned).first);
      expect(panel, const Rect.fromLTWH(990, 632, 580, 176));
      final selection = tester.getRect(
        find.byWidgetPredicate(
          (w) =>
              w is DecoratedBox &&
              (w.decoration as BoxDecoration).color == selectionColor,
        ),
      );
      // Nel riferimento x 1153–1272, y 660–779 (misure a ±1 pt).
      expect(selection.left, closeTo(1153, 1.5));
      expect(selection.top, closeTo(660, 1.5));
      expect(selection.size, const Size(120, 120));
      expect(find.text('App 1'), findsOneWidget);
    },
  );
}
