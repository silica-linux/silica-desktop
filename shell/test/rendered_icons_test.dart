// silica-icons 0.6: the Dock draws the rendered PNG of an app icon, keeps the clock hands and
// the calendar text dynamic on top, and an icon removed from the pack (store, help) falls back
// to the theme icon or the initial.

import 'dart:io';

import 'package:flutter/widgets.dart';
import 'package:flutter_svg/flutter_svg.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:silica_shell/dock/dock.dart';
import 'package:silica_shell/dock/model.dart';
import 'package:silica_shell/system/desktop_entries.dart';
import 'package:silica_shell/system/silica_icons.dart';
import 'package:silica_ui/silica_ui.dart';
import 'package:silica_ui/src/icons/silica_icon.dart' show RenderedIconImage;

const String svg =
    '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1024 1024">'
    '<rect x="100" y="100" width="824" height="824" fill="#2f8cff"/></svg>';

String manifest(String id, String dynamic) =>
    '''
{ "format": 1, "id": "$id", "name": "$id", "kind": "app", "keyline": "full",
  "layers": [ { "role": "background", "file": "background.svg" },
              { "role": "foreground", "file": "foreground.svg", "shadow": "neutral" } ],
  "variants": { "dark": { "background": "background-dark.svg",
                          "foreground": "foreground-dark.svg" } },
  "dynamic": $dynamic,
  "rendered": { "default": "icon.png", "dark": "icon-dark.png",
                "clear": "icon-clear.png", "clear-dark": "icon-clear-dark.png",
                "tinted": "icon-tinted.png", "tinted-dark": "icon-tinted-dark.png" } }''';

const String clockDynamic = '''
{ "type": "clock", "cap": "cap.svg", "hands": [
    { "unit": "hour", "file": "hand-hour.svg" },
    { "unit": "minute", "file": "hand-minute.svg" },
    { "unit": "second", "file": "hand-second.svg", "min_size": 64 } ] }''';

const String calendarDynamic = '''
{ "type": "calendar",
  "month": { "size": 92, "weight": 900, "color": "#f2382e", "dark_color": "#ff5248",
             "box": [100, 206, 824, 120], "case": "upper", "max_width": 560 },
  "day": { "size": 280, "weight": 700, "color": "#ffffff", "box": [100, 410, 824, 460] } }''';

/// A pack laid out like silica-icons 0.6: no `store` nor `help`. The PNGs are placeholders
/// (not decoded under the fake clock).
Directory pack() {
  final root = Directory.systemTemp.createTempSync('silica-icons-');
  void icon(String id, String dynamic, [Map<String, String> extra = const {}]) {
    final dir = Directory('${root.path}/apps/$id')..createSync(recursive: true);
    for (final MapEntry(:key, :value) in {
      'icon.json': manifest(id, dynamic),
      'background.svg': svg,
      'background-dark.svg': svg,
      'foreground.svg': svg,
      'foreground-dark.svg': svg,
      for (final suffix in [
        '',
        '-dark',
        '-clear',
        '-clear-dark',
        '-tinted',
        '-tinted-dark',
      ])
        'icon$suffix.png': 'png',
      ...extra,
    }.entries) {
      File('${dir.path}/$key').writeAsStringSync(value);
    }
  }

  icon('mail', 'null');
  icon('clock', clockDynamic, {
    'cap.svg': svg,
    'hand-hour.svg': svg,
    'hand-minute.svg': svg,
    'hand-second.svg': svg,
  });
  icon('calendar', calendarDynamic);
  return root;
}

void main() {
  late Directory root;
  late Map<String, String> env;
  setUp(() {
    root = pack();
    env = {'SILICA_ICONS_DIR': root.path};
  });
  tearDown(() => root.deleteSync(recursive: true));

  DesktopEntry entry(String id, String icon) => parseDesktopEntry(
    'dev.frascella.$id',
    '[Desktop Entry]\nType=Application\nName=$id\nExec=$id\nX-Silica-Icon=$icon',
  )!;

  Future<void> dock(WidgetTester tester, List<Item> items) async {
    tester.view.physicalSize = const Size(1280, 300);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.reset);
    await tester.pumpWidget(
      Directionality(
        textDirection: TextDirection.ltr,
        child: Dock(items: items, onActivate: (_) {}, menuFor: (_) => []),
      ),
    );
  }

  Finder inIcon(String label, Type type) => find.descendant(
    of: find.byWidgetPredicate(
      (w) =>
          w is SilicaIcon &&
          w.source is LayeredSource &&
          (w.source as LayeredSource).icon.id == label,
    ),
    matching: find.byType(type),
  );

  testWidgets('the Dock draws the rendered PNG of a Silica app', (
    tester,
  ) async {
    final mail = silicaIconFor(entry('Mail', 'mail'), env: env)!;
    await dock(tester, [Item.app('Mail', command: 'mail', layered: mail)]);
    final image = tester.widget<RenderedIconImage>(
      inIcon('mail', RenderedIconImage),
    );
    expect(image.path, endsWith('/apps/mail/icon.png'));
    // No SVG layer is composed under the PNG.
    expect(inIcon('mail', SvgPicture), findsNothing);
  });

  testWidgets('the system dark appearance keeps the Default PNG (MU-20)', (
    tester,
  ) async {
    tester.platformDispatcher.platformBrightnessTestValue = Brightness.dark;
    addTearDown(tester.platformDispatcher.clearPlatformBrightnessTestValue);
    final mail = silicaIconFor(entry('Mail', 'mail'), env: env)!;
    await dock(tester, [Item.app('Mail', command: 'mail', layered: mail)]);
    expect(
      tester.widget<RenderedIconImage>(inIcon('mail', RenderedIconImage)).path,
      endsWith('/icon.png'),
    );
  });

  testWidgets('Clock: hands and cap drawn over the PNG', (tester) async {
    final clock = silicaIconFor(entry('Clock', 'clock'), env: env)!;
    await dock(tester, [Item.app('Clock', command: 'clock', layered: clock)]);
    expect(inIcon('clock', RenderedIconImage), findsOneWidget);
    // Hour, minute and cap; the second hand only from 64 pt (the Dock rests at 48).
    expect(inIcon('clock', SvgPicture), findsNWidgets(3));
  });

  testWidgets('Calendar: month and day written over the PNG', (tester) async {
    final calendar = silicaIconFor(entry('Calendar', 'calendar'), env: env)!;
    await dock(tester, [
      Item.app('Calendar', command: 'calendar', layered: calendar),
    ]);
    expect(inIcon('calendar', RenderedIconImage), findsOneWidget);
    expect(inIcon('calendar', Text), findsNWidgets(2));
    expect(find.text('${DateTime.now().day}'), findsOneWidget);
  });

  test(
    'store and help left the pack: those apps fall back to their own icon',
    () {
      for (final id in ['store', 'help']) {
        final e = entry(id, id);
        expect(silicaIconFor(e, env: env), isNull);
        // The Dock then uses the theme icon, or the tile with the initial without one.
        final item = Item.app(
          id,
          command: id,
          layered: silicaIconFor(e, env: env),
        );
        expect(item.icon, isNull);
        final themed = Item.app(
          id,
          command: id,
          layered: silicaIconFor(e, env: env),
          iconPath: '/usr/share/icons/x.svg',
        );
        expect(themed.icon, isA<FlatSource>());
      }
    },
  );

  test('the shell entries name icons of the pack', () {
    final repo = Platform.environment['SILICA_ICONS_DIR'];
    final ids = [
      for (final file in Directory('linux').listSync())
        if (file.path.endsWith('.desktop.in'))
          RegExp(
            r'^X-Silica-Icon=(.+)$',
            multiLine: true,
          ).firstMatch(File(file.path).readAsStringSync())?.group(1),
    ].nonNulls.toList();
    expect(ids, containsAll(['apps', 'mission-control', 'screenshot']));
    // With a real silica-icons ≥ 0.6 at hand, each one exists and is rendered.
    if (repo == null ||
        !File('$repo/apps/mission-control/icon.json').existsSync()) {
      return;
    }
    for (final id in ids) {
      final icon = findSilicaIcon(id, env: {'SILICA_ICONS_DIR': repo});
      expect(icon, isNotNull, reason: id);
      expect(icon!.renderedFor(IconAppearance.light), isNotNull, reason: id);
    }
  });
}
