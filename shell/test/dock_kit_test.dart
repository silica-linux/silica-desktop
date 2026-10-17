// The Dock against the Amalfi kit (silica-design 41cc8f09, `kit/symbols/Dock/*`): the frames of
// `test/fixtures/kit/*.json` are the binding numbers. At the default canvas (60) every tile, icon,
// separator and dot frame of the laid-out Dock, and the label and the menu as rendered, equal
// the kit's in whole points; at 16 and 128 they follow the scaling rule (k = canvas / 60).

import 'dart:convert';
import 'dart:io';

import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:silica_shell/dock/arrow.dart';
import 'package:silica_shell/dock/dock.dart';
import 'package:silica_shell/dock/dock_menu.dart';
import 'package:silica_shell/dock/model.dart';
import 'package:silica_ui/silica_ui.dart';

Map<String, dynamic> kit(String name) =>
    jsonDecode(File('test/fixtures/kit/$name.json').readAsStringSync())
        as Map<String, dynamic>;

Rect frameOf(Map<String, dynamic> node) {
  final f = node['frame'] as Map<String, dynamic>;
  return Rect.fromLTWH(
    (f['x'] as num).toDouble(),
    (f['y'] as num).toDouble(),
    (f['w'] as num).toDouble(),
    (f['h'] as num).toDouble(),
  );
}

Map<String, dynamic> child(Map<String, dynamic> node, String name) =>
    (node['children'] as List).cast<Map<String, dynamic>>().singleWhere(
      (c) => c['name'] == name,
    );

/// The items of the kit's `Dock/Shelf/Default`, left to right, by the kit's instance name.
const kitOrder = [
  'Launchpad',
  'Mail',
  'Reminders',
  'Photos',
  'Notes',
  'Terminal',
  'Settings',
  'Separator',
  'Downloads stack',
  'Trash',
];

final kitItems = [
  const Item.app('Launchpad', command: 'a'),
  const Item.app('Mail', command: 'a', running: true),
  const Item.app('Reminders', command: 'a'),
  const Item.app('Photos', command: 'a'),
  const Item.app('Notes', command: 'a'),
  const Item.app('Terminal', command: 'a', running: true),
  const Item.app('Settings', command: 'a'),
  const Item.separator(),
  const Item.app('Downloads', folder: '/tmp'),
  const Item.app('Trash', trash: true),
];

/// A frame measured in the laid-out Dock, relative to the shelf's top left.
class Frames {
  Frames(this.canvas) {
    final m = DockMetrics(canvas);
    l = layout(kitItems, width: 800, height: 200, metrics: m);
    final (bx, by, bw, bh) = l.background;
    shelf = Rect.fromLTWH(0, 0, bw, bh);
    origin = Offset(bx, by);
    this.m = m;
  }

  final double canvas;
  late final Layout l;
  late final DockMetrics m;
  late final Rect shelf;
  late final Offset origin;

  Rect tile(int i) =>
      Rect.fromLTWH(l.cells[i].$1 - origin.dx, 0, l.cells[i].$2, shelf.height);

  Rect icon(int i) {
    final (x, y, side) = iconRect(l, i, 200);
    return Rect.fromLTWH(x - origin.dx, y - origin.dy, side, side);
  }

  /// The place or app icon as drawn (canvas of the 1024 drawing) in the shelf.
  Rect drawn(int i) {
    final (x, y, side) = iconRect(l, i, 200);
    final (bx, by, body) = iconBody(side, place: kitItems[i].isPlace);
    final canvasSide = kitItems[i].isPlace ? body : body * 1024 / 824;
    final cx = x + bx + body / 2, cy = y + by + body / 2;
    return Rect.fromCenter(
      center: Offset(cx - origin.dx, cy - origin.dy),
      width: canvasSide,
      height: canvasSide,
    );
  }

  /// The separator's line: left, top, 1 x height (the painter's own formula).
  Rect line(int i) {
    final left =
        l.center(i) - m.separatorWidth / 2 + m.separatorLine - origin.dx;
    return Rect.fromLTWH(
      left.roundToDouble(),
      m.separatorInset,
      1,
      m.separatorHeight,
    );
  }

  /// The running dot (the painter's own formula).
  Rect dot(int i) => Rect.fromCenter(
    center: Offset(l.center(i) - origin.dx, shelf.height - m.dotAbove),
    width: m.dot,
    height: m.dot,
  );
}

void expectRect(Rect actual, Rect expected, String what) {
  expect(actual.left, closeTo(expected.left, 1e-9), reason: '$what left');
  expect(actual.top, closeTo(expected.top, 1e-9), reason: '$what top');
  expect(actual.width, closeTo(expected.width, 1e-9), reason: '$what width');
  expect(actual.height, closeTo(expected.height, 1e-9), reason: '$what height');
}

void main() {
  setUp(() {
    TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
        .setMockMethodCallHandler(GlassRegistry.channel, (call) async => true);
  });

  test('the default canvas is the kit\'s 60', () {
    expect(defaultCanvas, 60);
    expect(minCanvas, 16);
    expect(maxCanvas, 128);
  });

  test('Dock/Shelf/Default at canvas 60: shelf, tiles, icons, separator, dots', () {
    final shelfKit = kit('shelf-default');
    final f = Frames(60);
    expectRect(
      f.shelf,
      frameOf(shelfKit).shift(-frameOf(shelfKit).topLeft),
      'shelf',
    );
    expect(f.shelf.size, const Size(620, 80));
    expect(f.m.radius, 24);
    final surface = child(shelfKit, 'Surface');
    expect(((surface['style'] as Map)['corners'] as Map)['radii'], [
      24,
      24,
      24,
      24,
    ]);
    // The tiles are the Mail/Trash masters; the icon and the dot are inside them.
    final tileKit = frameOf(kit('tile-running'));
    expect(tileKit.size, const Size(64, 80));
    final mailIcon = frameOf(child(kit('tile-running'), 'Icon'));
    final dotKit = frameOf(child(kit('tile-running'), 'Running dot'));
    final trashIcon = frameOf(child(kit('tile-trash'), 'Icon'));
    expect(mailIcon, const Rect.fromLTWH(2, 10, 60, 60));
    expect(trashIcon, mailIcon, reason: 'places have the same canvas as apps');
    expect(dotKit, const Rect.fromLTWH(30, 72, 4, 4));
    expect(frameOf(kit('dot')).size, const Size(4, 4));
    final sepKit = kit('separator-light');
    expect(frameOf(sepKit).size, const Size(28, 80));
    final lineKit = frameOf(child(sepKit, 'Line'));
    expect(lineKit, const Rect.fromLTWH(14, 10, 1, 60));

    for (final (i, name) in kitOrder.indexed) {
      final instance = child(shelfKit, name);
      final slot = frameOf(instance);
      expectRect(f.tile(i), slot, 'tile $name');
      if (name == 'Separator') {
        expectRect(f.line(i), lineKit.shift(slot.topLeft), 'line');
        continue;
      }
      expectRect(f.icon(i), mailIcon.shift(slot.topLeft), 'icon $name');
      // Places draw on the same canvas as apps: the drawing's canvas is the kit's 60 square.
      expectRect(f.drawn(i), mailIcon.shift(slot.topLeft), 'canvas $name');
      if (kitItems[i].running) {
        expectRect(f.dot(i), dotKit.shift(slot.topLeft), 'dot $name');
      }
    }
    // Icons are centred in their tile, both ways.
    final icon = f.icon(0);
    expect(icon.center.dx, f.tile(0).center.dx);
    expect(icon.center.dy, f.shelf.center.dy);
    // The bottom of the shelf is 4 pt above the screen bottom.
    expect(200 - (f.origin.dy + f.shelf.height), 4);
  });

  for (final canvas in [16.0, 128.0]) {
    test('scaling at canvas ${canvas.round()} follows k = canvas / 60', () {
      final k = canvas / 60;
      double r(double v) => (v * k).roundToDouble();
      final f = Frames(canvas);
      final pad = r(8), sep = r(28);
      final tile = canvas + 2 * r(2);
      final tileH = canvas + 2 * r(10);
      expect(f.shelf.width, 2 * pad + 9 * tile + sep);
      expect(f.shelf.height, tileH);
      expect(f.m.radius, r(24));
      expect(200 - (f.origin.dy + f.shelf.height), 4);
      for (final (i, name) in kitOrder.indexed) {
        if (name == 'Separator') {
          expect(f.tile(i).width, sep);
          expect(f.line(i).left - f.tile(i).left, r(14));
          expect(f.line(i).top, r(10));
          expect(f.line(i).height, tileH - 2 * r(10));
          continue;
        }
        // Centred in the tile: 2k each side (the tile is canvas + 4k), 10k from the top.
        expectRect(
          f.icon(i),
          Rect.fromLTWH(f.tile(i).left + r(2), r(10), canvas, canvas),
          'icon $name',
        );
        expect(f.tile(i).width, tile);
        if (kitItems[i].running) {
          final dot = f.dot(i);
          expect(dot.width, canvas == 16 ? 3 : r(4));
          expect(f.shelf.height - dot.center.dy, r(6));
          expect(dot.center.dx, f.tile(i).center.dx);
        }
      }
      expect(f.tile(0).left, pad);
      expect(f.tile(9).right, f.shelf.width - pad);
    });
  }

  testWidgets(
    'Dock/Label: body 28, radius 8, text + 32 wide, arrow 6 deep and 12 wide',
    (tester) async {
      final labelKit = kit('label');
      final master = frameOf(labelKit);
      expect(master.size, const Size(64, 34));
      final surface = child(labelKit, 'Surface');
      expect(((surface['style'] as Map)['corners'] as Map)['radii'], [
        8,
        8,
        8,
        3,
        2,
        3,
        8,
      ]);
      final textKit = frameOf(child(labelKit, 'Label'));
      // "Mail" is 32 wide in the kit: Ahem at 8 pt is 4 x 8.
      const style = TextStyle(fontSize: 8);
      final width = dockLabelWidth('Mail', style);
      expect(width, master.width);
      await tester.pumpWidget(
        Directionality(
          textDirection: TextDirection.ltr,
          child: Align(
            alignment: Alignment.topLeft,
            child: SizedBox(
              width: width,
              height: labelHeight + labelTip,
              child: const DockLabel(text: 'Mail', style: style, arrow: true),
            ),
          ),
        ),
      );
      await tester.pump();
      expect(tester.getSize(find.byType(DockLabel)), master.size);
      final glass = tester.renderObject<RenderGlass>(
        find.byWidgetPredicate((w) => w is Glass),
      );
      expect(glass.size, const Size(64, 28));
      expect(glass.radius, 8);
      final tip = glass.tip!;
      expect((tip.x, tip.width, tip.height), (32.0, 12.0, 6.0));
      final text = tester.getRect(find.text('Mail'));
      expect(text.left, textKit.left);
      expect(text.width, textKit.width);
      // The outline: body 64 x 28 and an arrow whose tip is at the bottom of the 34 frame.
      final path = const DockArrowBorder(
        radius: 8,
      ).getOuterPath(Offset.zero & master.size);
      expect(path.getBounds().left, 0);
      expect(path.getBounds().right, 64);
      // The kit's own outline reaches y 33.2 at the rounded tip (radius 2).
      expect(path.getBounds().bottom, closeTo(33.2042, 0.05));
      expect(path.contains(const Offset(32, 33)), isTrue);
      expect(path.contains(const Offset(32 - 9, 30)), isFalse);
      expect(labelHeight + labelTip, 34);
    },
  );

  testWidgets(
    'Dock/Menu: 200 x 110, body radius 12, rows 24 tall with 4 padding, arrow 6',
    (tester) async {
      final menuKit = kit('menu');
      final master = frameOf(menuKit);
      expect(master.size, const Size(200, 110));
      final surface = child(menuKit, 'Surface');
      expect(((surface['style'] as Map)['corners'] as Map)['radii'], [
        12,
        12,
        12,
        3,
        2,
        3,
        12,
      ]);
      final rowKit = frameOf(child(menuKit, 'Show in Mustache'));
      expect(rowKit, const Rect.fromLTWH(4, 28, 192, 24));
      final key = GlobalKey<DockMenuHostState>();
      await tester.pumpWidget(
        Directionality(
          textDirection: TextDirection.ltr,
          child: MediaQuery(
            data: const MediaQueryData(),
            child: DockMenuHost(
              key: key,
              entries: [
                MenuAction(
                  'Options',
                  onSelected: () {},
                  submenu: [MenuAction('A', onSelected: () {})],
                ),
                MenuAction('Show in Mustache', onSelected: () {}),
                MenuAction('Hide', onSelected: () {}),
                MenuAction('Quit', onSelected: () {}),
              ],
              anchorX: 300,
              bottom: 400,
              group: 0,
            ),
          ),
        ),
      );
      await tester.pump(const Duration(milliseconds: 300));
      await tester.pump(const Duration(milliseconds: 300));
      final rect = key.currentState!.rect!;
      expect(rect.height, master.height);
      expect(rect.width, greaterThanOrEqualTo(AmalfiMenuMetrics.minWidth));
      // The arrow's tip is on the anchor and at the bottom of the frame.
      expect(rect.bottom, 400);
      expect(rect.center.dx, 300);
      final glass = tester.renderObject<RenderGlass>(
        find.byWidgetPredicate((w) => w is Glass),
      );
      expect(glass.size.height, 104);
      expect(glass.radius, 12);
      final tip = glass.tip!;
      expect((tip.x, tip.width, tip.height), (rect.width / 2, 12.0, 6.0));
      // The rows: 24 tall, 4 from the sides and from the top (row 2 at y 28).
      final show = tester.getRect(find.text('Show in Mustache'));
      expect(show.top - rect.top, greaterThanOrEqualTo(28));
      expect(show.bottom - rect.top, lessThanOrEqualTo(52));
      expect(AmalfiMenuMetrics.padding, 4);
      expect(AmalfiMenuMetrics.radius, 12);
    },
  );
}
