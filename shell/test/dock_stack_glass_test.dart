import 'dart:io';

import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:silica_shell/dock/dock.dart';
import 'package:silica_shell/dock/model.dart';
import 'package:silica_shell/dock/stack.dart';
import 'package:silica_shell/dock/stack_settings.dart';
import 'package:silica_shell/system/silica_symbol.dart';
import 'package:silica_ui/silica_ui.dart';

// The Dock stacks (#162-#164): the Downloads folder end to end, the fan's spacing and glass
// labels, the grid's glass tip, the glass close and open-folder controls.
/// Any glass, the shell's `ProfiledGlass` included (`find.byType` matches the exact type).
final anyGlass = find.byWidgetPredicate((w) => w is Glass);

void main() {
  setUp(() {
    TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
        .setMockMethodCallHandler(GlassRegistry.channel, (call) async => true);
  });

  late Directory dir;
  late List<String> opened;

  Future<void> openStack(
    WidgetTester tester,
    int files, {
    StackSettings settings = const StackSettings(),
    void Function(Directory)? prepare,
  }) async {
    dir = Directory.systemTemp.createTempSync('Downloads');
    addTearDown(() => dir.deleteSync(recursive: true));
    for (var i = 1; i <= files; i++) {
      File('${dir.path}/documento-$i.txt').writeAsStringSync('x');
    }
    prepare?.call(dir);
    tester.view.physicalSize = const Size(2560, 1440);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.reset);
    opened = [];
    final items = [Item.app('Downloads', command: 'x', folder: dir.path)];
    await tester.pumpWidget(
      Directionality(
        textDirection: TextDirection.ltr,
        child: Dock(
          items: items,
          onOpenPath: opened.add,
          stackSettingsFor: (_) => settings,
        ),
      ),
    );
    final rest = layout(items, width: 2560, height: 1440);
    await tester.tapAt(
      Offset(rest.center(0), 1440 - bottomGap - backgroundHeight / 2),
    );
    for (var i = 0; i < 6; i++) {
      await tester.pump(const Duration(milliseconds: 60));
    }
  }

  testWidgets('Downloads: opens and lists its files in the configured order', (
    tester,
  ) async {
    await openStack(
      tester,
      0,
      settings: const StackSettings(sort: StackSort.kind),
      prepare: (d) {
        for (final name in ['b.zip', 'a.txt', 'c.md', '.hidden']) {
          File('${d.path}/$name').writeAsStringSync('x');
        }
        Directory('${d.path}/folder').createSync();
      },
    );
    // By kind: the folder, then md, txt, zip; the hidden file is not listed.
    final names = ['folder', 'c.md', 'a.txt', 'b.zip'];
    final ys = [for (final n in names) tester.getCenter(find.text(n)).dy];
    // The first is at the bottom, next to the Dock.
    expect(ys, orderedEquals([...ys]..sort((a, b) => b.compareTo(a))));
    expect(find.text('.hidden'), findsNothing);
    expect(find.byType(FanStack), findsOneWidget);
    await tester.tap(find.text('c.md'));
    for (var i = 0; i < 6; i++) {
      await tester.pump(const Duration(milliseconds: 50));
    }
    expect(opened, ['${dir.path}/c.md']);
  });

  testWidgets('fan: items further apart along the arc, labels are glass', (
    tester,
  ) async {
    await openStack(tester, 4);
    double top(int i) => fanSlot(i).center.dy - fanSlot(i).side / 2;
    expect((top(0) - top(1)).abs(), closeTo(fanPitch * fanSpread, 0.01));
    expect(fanSpread, greaterThan(1));
    final labels = find.byType(DockLabel);
    // Four files and "Open folder".
    expect(labels, findsNWidgets(5));
    for (final label in tester.widgetList<DockLabel>(labels)) {
      expect(label.tip, isFalse);
    }
    // Their glass is the Dock label's and the menus': thick, no tip.
    final glasses = tester
        .renderObjectList<RenderGlass>(
          find.descendant(of: labels, matching: anyGlass),
        )
        .toList();
    expect(glasses, hasLength(5));
    for (final glass in glasses) {
      expect(glass.material, DockLabel.material);
      expect(glass.tip, isNull);
    }
    // And the labels sit to the left of their icon.
    final label = tester.getRect(find.byType(DockLabel).first);
    expect(tester.getSize(find.byType(DockLabel).first).height, labelHeight);
    expect(
      label.right,
      lessThan(
        tester.getRect(find.byKey(const ValueKey('fan icon 0'))).left + 1,
      ),
    );
  });

  testWidgets('grid: the panel has the same glass tip as the menus', (
    tester,
  ) async {
    await openStack(tester, 40);
    expect(find.byType(GridStack), findsOneWidget);
    final panel = tester
        .renderObjectList<RenderGlass>(
          find.descendant(
            of: find.byType(GridStack),
            matching: anyGlass,
          ),
        )
        .first;
    final tip = panel.tip!;
    expect(panel.material, GlassMaterial.thick);
    expect(tip.edge, TipEdge.bottom);
    expect(tip.width, MenuMetrics.tipBase);
    expect(tip.height, MenuMetrics.tipDepth);
    // The menu declares the same rectangle.
    await tester.pumpWidget(
      Directionality(
        textDirection: TextDirection.ltr,
        child: SizedBox(
          width: 200,
          height: 80,
          child: MenuPanel(
            entries: [MenuAction('Open', onSelected: () {})],
            tipX: 100,
          ),
        ),
      ),
    );
    final menu = tester.renderObject<RenderGlass>(anyGlass);
    expect(menu.tip!.width, tip.width);
    expect(menu.tip!.height, tip.height);
  });

  testWidgets('close button: a glass circle with the chevron symbol', (
    tester,
  ) async {
    await openStack(tester, 3);
    final button = find.byType(StackCloseButton);
    expect(button, findsOneWidget);
    final glass = tester.renderObject<RenderGlass>(
      find.descendant(of: button, matching: anyGlass),
    );
    final circle = tester.getRect(
      find.descendant(of: button, matching: anyGlass),
    );
    expect(circle.width, circle.height);
    expect(glass.radius, closeTo(circle.width / 2, 1e-6));
    expect(
      find.descendant(of: button, matching: find.byType(SilicaSymbol)),
      findsOneWidget,
    );
    expect(
      tester
          .widget<SilicaSymbol>(
            find.descendant(of: button, matching: find.byType(SilicaSymbol)),
          )
          .name,
      'chevron-down',
    );
    // No layered icon (Sketch artwork) inside.
    expect(
      find.descendant(of: button, matching: find.byType(SilicaIcon)),
      findsNothing,
    );
    await tester.tap(button);
    for (var i = 0; i < 6; i++) {
      await tester.pump(const Duration(milliseconds: 50));
    }
    expect(find.byType(FanStack), findsNothing);
    expect(find.byType(StackCloseButton), findsNothing);
  });

  testWidgets(
    'open folder: a glass circle with the arrow, it opens the folder',
    (tester) async {
      await openStack(tester, 3);
      final button = find.byType(OpenFolderButton);
      expect(button, findsOneWidget);
      expect(
        find.descendant(of: button, matching: anyGlass),
        findsOneWidget,
      );
      expect(
        tester
            .widget<SilicaSymbol>(
              find.descendant(of: button, matching: find.byType(SilicaSymbol)),
            )
            .name,
        'arrow-up-right',
      );
      await tester.tap(button);
      for (var i = 0; i < 6; i++) {
        await tester.pump(const Duration(milliseconds: 50));
      }
      expect(opened, [dir.path]);
    },
  );

  testWidgets('grid: the open-folder cell is the glass button too', (
    tester,
  ) async {
    await openStack(
      tester,
      5,
      settings: const StackSettings(content: StackContent.grid),
    );
    expect(find.byType(OpenFolderButton), findsOneWidget);
    await tester.tap(find.byType(OpenFolderButton));
    for (var i = 0; i < 6; i++) {
      await tester.pump(const Duration(milliseconds: 50));
    }
    expect(opened, [dir.path]);
  });

  test('a silica-icons symbol is read from symbols/<weight>', () {
    final root = Directory.systemTemp.createTempSync('icons');
    addTearDown(() => root.deleteSync(recursive: true));
    Directory('${root.path}/symbols/regular').createSync(recursive: true);
    File('${root.path}/symbols/regular/chevron-down.svg').writeAsStringSync(
      '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"/>',
    );
    final env = {'SILICA_ICONS_DIR': root.path, 'HOME': root.path};
    expect(symbolSvg('chevron-down', env: env), contains('<svg'));
    expect(symbolSvg('missing', env: env), isNull);
    expect(symbolSvg('../x', env: env), isNull);
  });
}
