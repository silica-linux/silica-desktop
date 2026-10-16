// The shell's Amalfi menus, popovers, alerts and tooltips use Ossido's glass
// (`AmalfiGlassBackend.compositor`): each surface declares a compositor glass region and
// Flutter paints no tint, rim, shadow or backdrop of its own. (The Options menu and the
// authentication dialog are in screenshot_test.dart and auth_dialog_test.dart.)

import 'dart:ui' show PointerDeviceKind;

import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:silica_shell/controlcenter/hud.dart';
import 'package:silica_shell/dock/dock.dart' show Dock;
import 'package:silica_shell/faro/faro.dart' show Faro;
import 'package:silica_shell/dock/dock_menu.dart';
import 'package:silica_shell/main.dart' show ShellComponent;
import 'package:silica_shell/menubar/bar_menu_host.dart';
import 'package:silica_shell/menubar/power.dart';
import 'package:silica_shell/menubar/window_menu.dart';
import 'package:silica_ui/silica_ui.dart';

import 'controlcenter_test.dart' show FakeSystem, laptop;
import 'glass_backend_probe.dart';

Widget _scene(Widget child) => Directionality(
  textDirection: TextDirection.ltr,
  child: compositorScope(Stack(children: [Positioned.fill(child: child)])),
);

void main() {
  setUp(() {
    TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
        .setMockMethodCallHandler(GlassRegistry.channel, (call) async => true);
  });

  testWidgets('menu bar: the menu and its submenu are menu-kind regions', (
    tester,
  ) async {
    final reports = <List<Rect>>[];
    await tester.pumpWidget(
      _scene(
        BarMenuHost(
          entries: [
            const AmalfiMenuAction('Plain'),
            AmalfiMenuAction('More', submenu: const [AmalfiMenuAction('In')]),
          ],
          anchor: const Offset(50, 36),
          onRectsChanged: reports.add,
        ),
      ),
    );
    await tester.pump(const Duration(milliseconds: 100));
    expect(glassProfiles(tester), ['menu']);
    expectNoFlutterTint(tester);
    final mouse = await tester.createGesture(kind: PointerDeviceKind.mouse);
    addTearDown(mouse.removePointer);
    await mouse.addPointer(location: Offset.zero);
    await mouse.moveTo(tester.getCenter(find.text('More')));
    await tester.pump(const Duration(milliseconds: 400));
    await tester.pump(const Duration(milliseconds: 100));
    // The submenu is a region of its own, in a group above its menu; the host learns its
    // rectangle from the menu (`onSubmenusChanged`), no polling.
    expect(glassProfiles(tester), ['menu', 'menu']);
    final groups = [
      for (final g in tester.renderObjectList<RenderGlass>(
        find.byWidgetPredicate((w) => w is Glass),
      ))
        g.group,
    ];
    expect(groups.toSet(), {0, 1});
    expect(reports.last, hasLength(2));
    expectNoFlutterTint(tester);
  });

  testWidgets('power alert: a sheet-kind region', (tester) async {
    await tester.pumpWidget(
      Directionality(
        textDirection: TextDirection.ltr,
        child: compositorScope(
          Center(
            child: PowerConfirmation(
              action: PowerAction.shutDown,
              user: 'Alex',
              onConfirm: () {},
              onCancel: () {},
            ),
          ),
        ),
      ),
    );
    await tester.pump(const Duration(milliseconds: 300));
    expect(glassProfiles(tester), ['sheet']);
    expectNoFlutterTint(tester);
  });

  testWidgets(
    'window popover: a popover-kind region with the arrow as its tip',
    (tester) async {
      await tester.pumpWidget(
        _scene(WindowMenuPopover(tip: const Offset(300, 40), execute: (_) {})),
      );
      await tester.pump(const Duration(milliseconds: 500));
      expect(glassProfiles(tester), ['popover']);
      final glass = tester.widget<Glass>(find.byType(Glass));
      expect(glass.tip, isNotNull);
      expectNoFlutterTint(tester);
    },
  );

  testWidgets('Dock menu: its submenu goes through onSubmenusChanged', (
    tester,
  ) async {
    final reports = <List<Rect>>[];
    await tester.pumpWidget(
      _scene(
        DockMenuHost(
          entries: [
            MenuAction(
              'Options',
              onSelected: () {},
              submenu: [MenuAction('A', onSelected: () {})],
            ),
            MenuAction('Quit', onSelected: () {}),
          ],
          anchorX: 300,
          bottom: 400,
          group: 0,
          onSubmenusChanged: reports.add,
        ),
      ),
    );
    await tester.pump(const Duration(milliseconds: 300));
    // The body is the Dock's own glass (its arrow outline is the Dock's), under the rows.
    expect(glassProfiles(tester), ['menu']);
    expectNoFlutterTint(tester);
    final mouse = await tester.createGesture(kind: PointerDeviceKind.mouse);
    addTearDown(mouse.removePointer);
    await mouse.addPointer(location: Offset.zero);
    await mouse.moveTo(tester.getCenter(find.text('Options')));
    await tester.pump(const Duration(milliseconds: 400));
    await tester.pump(const Duration(milliseconds: 100));
    expect(reports.last, hasLength(1));
    // The submenu is Ossido's glass too, one group above the menu.
    expect(glassProfiles(tester), ['menu', 'menu']);
    final groups = [
      for (final g in tester.renderObjectList<RenderGlass>(
        find.byWidgetPredicate((w) => w is Glass),
      ))
        g.group,
    ];
    expect(groups.toSet(), {0, 1});
    expectNoFlutterTint(tester);
  });

  testWidgets('HUD: the level is a read-only AmalfiSlider, no thumb', (
    tester,
  ) async {
    tester.view.physicalSize = const Size(1280, 800);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.reset);
    final system = FakeSystem(laptop.copyWith());
    await tester.pumpWidget(
      Directionality(
        textDirection: TextDirection.ltr,
        child: compositorScope(SystemHud(system: system, suppressed: false)),
      ),
    );
    system.value = system.value.copyWith(volume: 0.5);
    await tester.pump();
    await tester.pump(hudAppear);
    final slider = tester.widget<AmalfiSlider>(find.byType(AmalfiSlider));
    expect(slider.indicator, isTrue);
    expect(slider.value, 0.5);
    expect(find.byType(AmalfiSliderThumb), findsNothing);
    expectGlassRegion(tester, 'control');
    expectNoFlutterTint(tester);
  });

  testWidgets('a shell layer is under the compositor backend, Faro is not', (
    tester,
  ) async {
    tester.view.physicalSize = const Size(1280, 800);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.reset);
    await tester.pumpWidget(const ShellComponent(component: 'dock-demo'));
    expect(
      AmalfiGlassBackendScope.of(tester.element(find.byType(Dock))),
      AmalfiGlassBackend.compositor,
    );
    await tester.pumpWidget(const ShellComponent(component: 'faro'));
    expect(
      AmalfiGlassBackendScope.of(tester.element(find.byType(Faro))),
      AmalfiGlassBackend.flutter,
    );
  });
}
