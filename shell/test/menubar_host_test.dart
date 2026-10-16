import 'dart:ui' show PointerDeviceKind;

import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:silica_shell/menubar/bar_menu_host.dart';
import 'package:silica_shell/menubar/menubar.dart';
import 'package:silica_shell/system/shell_channel.dart';
import 'package:silica_ui/silica_ui.dart';

Widget _scene(Widget child) => Directionality(
  textDirection: TextDirection.ltr,
  child: Stack(children: [Positioned.fill(child: child)]),
);

void main() {
  test('the model of the shell becomes Amalfi rows', () {
    final chosen = <String>[];
    final rows = amalfiEntries([
      const MenuSection('Share'),
      MenuAction('Open', shortcut: '⌘O', onSelected: () => chosen.add('open')),
      const MenuAction('Off', enabled: false, checked: true),
      const MenuDivider(),
      MenuAction('More', submenu: [MenuAction('Inner', onSelected: () {})]),
      MenuCustom(
        height: 10,
        width: 10,
        builder: (context, close) => const SizedBox(),
      ),
    ]);
    // A custom row has no Amalfi row.
    expect(rows, hasLength(5));
    expect(rows[0], isA<AmalfiMenuSection>());
    final open = rows[1] as AmalfiMenuAction;
    expect((open.label, open.shortcut), ('Open', '⌘O'));
    open.onSelected?.call();
    expect(chosen, ['open']);
    final off = rows[2] as AmalfiMenuAction;
    expect((off.enabled, off.checked), (false, true));
    expect(rows[3], isA<AmalfiMenuDivider>());
    expect((rows[4] as AmalfiMenuAction).submenu, hasLength(1));
  });

  group('BarMenuHost', () {
    testWidgets('fades in and out with fade.instant, without scale', (
      tester,
    ) async {
      var gone = 0;
      await tester.pumpWidget(
        _scene(
          BarMenuHost(
            entries: const [AmalfiMenuAction('One'), AmalfiMenuAction('Two')],
            anchor: const Offset(100, 36),
            onDismissed: () => gone++,
          ),
        ),
      );
      double opacity() => tester
          .widget<FadeTransition>(find.byType(FadeTransition).first)
          .opacity
          .value;
      expect(opacity(), 0);
      await tester.pump(AmalfiFades.instant.duration);
      expect(opacity(), 1);
      // No scale anywhere around the menu (motion.md).
      expect(
        find.ancestor(
          of: find.byType(AmalfiMenu),
          matching: find.byType(ScaleTransition),
        ),
        findsNothing,
      );
      // The menu is at the anchor.
      expect(tester.getTopLeft(find.byType(AmalfiMenu)), const Offset(100, 36));
      // Esc fades it out, then reports.
      await tester.sendKeyEvent(LogicalKeyboardKey.escape);
      await tester.pump();
      expect(gone, 0);
      await tester.pump(AmalfiFades.instant.duration ~/ 2);
      expect(opacity(), allOf(greaterThan(0), lessThan(1)));
      await tester.pump(AmalfiFades.instant.duration);
      expect(gone, 1);
    });

    testWidgets('a choice blinks, runs and closes; a press outside closes', (
      tester,
    ) async {
      final chosen = <String>[];
      var gone = 0;
      final key = GlobalKey<BarMenuHostState>();
      await tester.pumpWidget(
        _scene(
          BarMenuHost(
            key: key,
            entries: [
              AmalfiMenuAction('One', onSelected: () => chosen.add('one')),
            ],
            anchor: const Offset(20, 36),
            onDismissed: () => gone++,
          ),
        ),
      );
      await tester.pump(AmalfiFades.instant.duration);
      await tester.tap(find.text('One'));
      await tester.pump(const Duration(milliseconds: 120));
      expect(chosen, ['one']);
      await tester.pump(AmalfiFades.instant.duration * 2);
      expect(gone, 1);

      // A press outside the menu: closes.
      gone = 0;
      await tester.pumpWidget(
        _scene(
          BarMenuHost(
            key: GlobalKey<BarMenuHostState>(),
            entries: const [AmalfiMenuAction('One')],
            anchor: const Offset(20, 36),
            onDismissed: () => gone++,
          ),
        ),
      );
      await tester.pump(AmalfiFades.instant.duration);
      final host = tester.state<BarMenuHostState>(find.byType(BarMenuHost));
      host.pointerDown(const Offset(600, 300));
      await tester.pump();
      await tester.pump(AmalfiFades.instant.duration * 2);
      expect(gone, 1);
    });

    testWidgets('reports the rectangles of the menu and of an open submenu', (
      tester,
    ) async {
      final reports = <List<Rect>>[];
      await tester.pumpWidget(
        _scene(
          BarMenuHost(
            entries: [
              const AmalfiMenuAction('Plain'),
              AmalfiMenuAction(
                'More',
                submenu: const [AmalfiMenuAction('Inner')],
              ),
            ],
            anchor: const Offset(50, 36),
            onRectsChanged: reports.add,
          ),
        ),
      );
      await tester.pump(const Duration(milliseconds: 100));
      expect(reports.last, hasLength(1));
      expect(reports.last.first.topLeft, const Offset(50, 36));
      // Resting on the item with a submenu opens it after ~295 ms.
      final mouse = await tester.createGesture(kind: PointerDeviceKind.mouse);
      addTearDown(mouse.removePointer);
      await mouse.addPointer(location: Offset.zero);
      await mouse.moveTo(tester.getCenter(find.text('More')));
      await tester.pump(const Duration(milliseconds: 400));
      await tester.pump();
      await tester.pump(const Duration(milliseconds: 100));
      expect(reports.last, hasLength(2));
      expect(reports.last[1].left, lessThan(reports.last[0].right));
    });

    testWidgets(
      '← and → go to the neighbouring title, unless → opens a submenu',
      (tester) async {
        var next = 0, previous = 0;
        await tester.pumpWidget(
          _scene(
            BarMenuHost(
              entries: [
                const AmalfiMenuAction('Plain'),
                AmalfiMenuAction(
                  'More',
                  submenu: const [AmalfiMenuAction('Inner')],
                ),
              ],
              anchor: const Offset(50, 36),
              onNext: () => next++,
              onPrevious: () => previous++,
            ),
          ),
        );
        await tester.pump(AmalfiFades.instant.duration);
        await tester.sendKeyEvent(LogicalKeyboardKey.arrowRight);
        await tester.pump();
        expect(next, 1);
        await tester.sendKeyEvent(LogicalKeyboardKey.arrowLeft);
        await tester.pump();
        expect(previous, 1);
        // On a row with a submenu the menu takes → and opens it.
        await tester.sendKeyEvent(LogicalKeyboardKey.arrowDown);
        await tester.sendKeyEvent(LogicalKeyboardKey.arrowDown);
        await tester.pump();
        await tester.sendKeyEvent(LogicalKeyboardKey.arrowRight);
        await tester.pump(const Duration(milliseconds: 50));
        expect(next, 1);
        expect(find.text('Inner'), findsOneWidget);
      },
    );

    testWidgets('a menu that does not fit slides in from the right edge', (
      tester,
    ) async {
      tester.view.physicalSize = const Size(400, 400);
      tester.view.devicePixelRatio = 1;
      addTearDown(tester.view.reset);
      await tester.pumpWidget(
        _scene(
          const BarMenuHost(
            entries: [AmalfiMenuAction('A fairly long item label')],
            anchor: Offset(390, 36),
          ),
        ),
      );
      await tester.pump(AmalfiFades.instant.duration);
      final rect = tester.getRect(find.byType(AmalfiMenu));
      expect(rect.right, lessThanOrEqualTo(400));
      expect(rect.left, greaterThanOrEqualTo(0));
    });
  });

  group('the bar and its menus', () {
    Future<void> pumpBar(WidgetTester tester) async {
      tester.view.physicalSize = const Size(1280, 400);
      tester.view.devicePixelRatio = 1;
      addTearDown(tester.view.reset);
      tester.binding.defaultBinaryMessenger
        ..setMockMethodCallHandler(ShellChannel.channel, (call) async => null)
        ..setMockMethodCallHandler(
          const MethodChannel('silica/layer', JSONMethodCodec()),
          (call) async => true,
        );
      await tester.pumpWidget(
        Directionality(
          textDirection: TextDirection.ltr,
          child: MenuBar(now: DateTime(2026, 10, 12, 22, 34)),
        ),
      );
      await tester.pump();
    }

    testWidgets(
      'the open title has the Accent Soft capsule and the accent text',
      (tester) async {
        await pumpBar(tester);
        final colors = AmalfiColors.of(Brightness.light);
        Color? fill(String text) {
          final box = find
              .ancestor(
                of: find.text(text),
                matching: find.byType(DecoratedBox),
              )
              .first;
          final decoration = tester.widget<DecoratedBox>(box).decoration;
          return decoration is ShapeDecoration ? decoration.color : null;
        }

        expect(fill('Window'), isNull);
        await tester.tap(find.text('Window'));
        await tester.pump();
        await tester.pump();
        expect(fill('Window'), const Color(0x66FFFFFF));
        expect(
          tester.widget<Text>(find.text('Window')).style?.color,
          colors.textPrimary,
        );
        expect(fill('View'), isNull);
      },
    );

    testWidgets(
      '→ and ← in an open menu open the next and the previous title',
      (tester) async {
        await pumpBar(tester);
        await tester.tap(find.text('Window'));
        await tester.pump();
        await tester.pump(const Duration(milliseconds: 150));
        expect(find.text('Minimize'), findsOneWidget);
        // The generic titles with items: File, View, Window, Help.
        await tester.sendKeyEvent(LogicalKeyboardKey.arrowRight);
        await tester.pump();
        await tester.pump(const Duration(milliseconds: 150));
        expect(find.text('Minimize'), findsNothing);
        expect(find.text('Silica Help'), findsOneWidget);
        // The new menu starts with its first row highlighted (keyboard).
        expect(
          tester
              .widgetList<AmalfiMenuItem>(find.byType(AmalfiMenuItem))
              .any((i) => i.highlighted),
          isTrue,
        );
        await tester.sendKeyEvent(LogicalKeyboardKey.arrowLeft);
        await tester.pump();
        await tester.pump(const Duration(milliseconds: 150));
        expect(find.text('Minimize'), findsOneWidget);
      },
    );

    testWidgets('moving the pointer onto another title switches after ~85 ms', (
      tester,
    ) async {
      await pumpBar(tester);
      await tester.tap(find.text('Window'));
      await tester.pump();
      await tester.pump(const Duration(milliseconds: 150));
      final mouse = await tester.createGesture(kind: PointerDeviceKind.mouse);
      addTearDown(mouse.removePointer);
      await mouse.addPointer(location: tester.getCenter(find.text('Window')));
      await mouse.moveTo(tester.getCenter(find.text('Help')));
      await tester.pump(const Duration(milliseconds: 40));
      expect(find.text('Silica Help'), findsNothing);
      await tester.pump(const Duration(milliseconds: 100));
      await tester.pump(const Duration(milliseconds: 100));
      expect(find.text('Silica Help'), findsOneWidget);
    });

    testWidgets(
      'Esc closes the menu with a fade and shrinks the surface back',
      (tester) async {
        final layer = <String>[];
        await pumpBar(tester);
        tester.binding.defaultBinaryMessenger.setMockMethodCallHandler(
          const MethodChannel('silica/layer', JSONMethodCodec()),
          (call) async {
            layer.add(call.method);
            return true;
          },
        );
        await tester.tap(find.text('Window'));
        await tester.pump();
        await tester.pump(const Duration(milliseconds: 150));
        expect(find.text('Minimize'), findsOneWidget);
        await tester.sendKeyEvent(LogicalKeyboardKey.escape);
        await tester.pump();
        // Still there while it fades.
        expect(find.text('Minimize'), findsOneWidget);
        await tester.pump(const Duration(milliseconds: 200));
        expect(find.text('Minimize'), findsNothing);
        expect(layer.last, 'setSize');
      },
    );
  });
}
