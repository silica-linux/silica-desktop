// An app that carries its own icon: `X-Silica-Icon=<absolute path of its icon/ folder>`
// (docs/assets-format.md, "App icons"). A plain name keeps the lookup in the icons folder.

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

const String manifest = '''
{ "format": 1, "id": "clock", "name": "Clock", "kind": "app", "keyline": "full",
  "layers": [ { "role": "background", "file": "background.svg" },
              { "role": "foreground", "file": "foreground.svg", "shadow": "neutral" } ],
  "variants": {},
  "dynamic": { "type": "clock", "cap": "cap.svg", "hands": [
    { "unit": "hour", "file": "hand-hour.svg" },
    { "unit": "minute", "file": "hand-minute.svg" } ] },
  "rendered": { "default": "icon.png", "dark": "icon-dark.png" } }''';

/// An app's `icon/` folder: the silica-icons app icon as it is.
Directory appIcon(Directory parent, {String name = 'icon'}) {
  final dir = Directory('${parent.path}/$name')..createSync(recursive: true);
  for (final MapEntry(:key, :value) in {
    'icon.json': manifest,
    'background.svg': svg,
    'foreground.svg': svg,
    'cap.svg': svg,
    'hand-hour.svg': svg,
    'hand-minute.svg': svg,
    'icon.png': 'png',
    'icon-dark.png': 'png',
  }.entries) {
    File('${dir.path}/$key').writeAsStringSync(value);
  }
  return dir;
}

DesktopEntry entry(String icon) => parseDesktopEntry(
  'dev.frascella.Clock',
  '[Desktop Entry]\nType=Application\nName=Clock\nExec=clock\n'
      'Icon=$icon/icon.png\nX-Silica-Icon=$icon',
)!;

void main() {
  late Directory root;
  setUp(() => root = Directory.systemTemp.createTempSync('silica-app-icon-'));
  tearDown(() => root.deleteSync(recursive: true));

  test('an absolute X-Silica-Icon loads the icon from that folder', () {
    final dir = appIcon(root);
    final icon = silicaIconFor(entry(dir.path), env: const {})!;
    expect(icon.id, 'clock');
    expect(icon.renderedFor(IconAppearance.light), '${dir.path}/icon.png');
    expect(icon.renderedFor(IconAppearance.dark), '${dir.path}/icon-dark.png');
    // Dynamic layers (clock hands and cap) come from the same folder.
    expect(icon.dynamic, isA<ClockHands>());
    // Trailing slash and the cache give the same icon.
    expect(silicaIconFor(entry('${dir.path}/'), env: const {})?.id, 'clock');
  });

  test('a folder without a valid icon gives null, never the name lookup', () {
    expect(silicaIconFor(entry('${root.path}/missing'), env: const {}), isNull);
    final broken = Directory('${root.path}/broken')..createSync();
    File('${broken.path}/icon.json').writeAsStringSync('{ not json');
    expect(silicaIconFor(entry(broken.path), env: const {}), isNull);
    // A manifest naming a missing layer file is invalid.
    final partial = appIcon(root, name: 'partial');
    File('${partial.path}/background.svg').deleteSync();
    expect(silicaIconFor(entry(partial.path), env: const {}), isNull);
    // Paths that climb out are refused.
    expect(appIconFromFolder('${root.path}/icon/../icon'), isNull);
    expect(appIconFromFolder('relative/icon'), isNull);
  });

  test('a folder that appears later is found', () {
    final path = '${root.path}/later';
    expect(appIconFromFolder(path), isNull);
    appIcon(root, name: 'later');
    expect(appIconFromFolder(path)?.id, 'clock');
  });

  test('a plain name keeps the lookup in the icons folder', () {
    final icons = Directory('${root.path}/icons');
    appIcon(Directory('${icons.path}/apps'), name: 'clock');
    final env = {'SILICA_ICONS_DIR': icons.path};
    final byName = parseDesktopEntry(
      'dev.frascella.Clock',
      '[Desktop Entry]\nType=Application\nName=Clock\nExec=clock\nX-Silica-Icon=clock',
    )!;
    expect(
      silicaIconFor(byName, env: env)?.renderedFor(IconAppearance.light),
      '${icons.path}/apps/clock/icon.png',
    );
    // Without a SILICA_ICONS_DIR holding it, the name is not found (as before).
    expect(
      silicaIconFor(byName, env: const {'XDG_DATA_DIRS': '/nonexistent'}),
      isNull,
    );
  });

  testWidgets(
    'the Dock draws the rendered PNG and the hands of an app folder',
    (tester) async {
      tester.view.physicalSize = const Size(1280, 300);
      tester.view.devicePixelRatio = 1;
      addTearDown(tester.view.reset);
      final dir = appIcon(root);
      final clock = silicaIconFor(entry(dir.path), env: const {})!;
      await tester.pumpWidget(
        Directionality(
          textDirection: TextDirection.ltr,
          child: Dock(
            items: [Item.app('Clock', command: 'clock', layered: clock)],
            onActivate: (_) {},
            menuFor: (_) => [],
          ),
        ),
      );
      final icon = find.byWidgetPredicate(
        (w) =>
            w is SilicaIcon &&
            w.source is LayeredSource &&
            (w.source as LayeredSource).icon.id == 'clock',
      );
      expect(
        tester
            .widget<RenderedIconImage>(
              find.descendant(
                of: icon,
                matching: find.byType(RenderedIconImage),
              ),
            )
            .path,
        '${dir.path}/icon.png',
      );
      // Hour and minute hands and the cap, over the PNG.
      expect(
        find.descendant(of: icon, matching: find.byType(SvgPicture)),
        findsNWidgets(3),
      );
    },
  );
}
