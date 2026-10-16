import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:silica_shell/dock/dock.dart';
import 'package:silica_shell/dock/dock_menu.dart';
import 'package:silica_shell/dock/glass_groups.dart';
import 'package:silica_ui/silica_ui.dart';

// The Dock label is an Amalfi tooltip (Glass/Thick, radius 8, 24 pt, no tip) drawn by Ossido,
// and the Dock menu a Thick menu surface (radius 12) over it in the glass order, light and
// dark.
void main() {
  setUp(() {
    TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
        .setMockMethodCallHandler(GlassRegistry.channel, (call) async => true);
  });

  for (final brightness in Brightness.values) {
    testWidgets('label and menu declare Thick glass (${brightness.name})', (
      tester,
    ) async {
      await tester.pumpWidget(
        MediaQuery(
          data: MediaQueryData(platformBrightness: brightness),
          child: Directionality(
            textDirection: TextDirection.ltr,
            child: Stack(
              children: [
                const Positioned(
                  left: 0,
                  top: 0,
                  width: 120,
                  height: 40,
                  child: DockLabel(text: 'Files', style: TextStyle()),
                ),
                Positioned.fill(
                  child: DockMenuHost(
                    entries: [MenuAction('Open', onSelected: () {})],
                    anchorX: 200,
                    bottom: 300,
                    group: 0,
                  ),
                ),
              ],
            ),
          ),
        ),
      );
      await tester.pump(const Duration(milliseconds: 200));
      final glasses = tester
          .renderObjectList<RenderGlass>(
            find.byWidgetPredicate((w) => w is Glass),
          )
          .toList();
      expect(glasses, hasLength(2));
      final label = glasses[0];
      final menu = glasses[1];
      expect(label.material, GlassMaterial.thick);
      expect(menu.material, GlassMaterial.thick);
      // Its own glass group, below the menu's: the two never merge (glass_groups.dart).
      expect(label.group, DockGlass.label);
      expect(label.group, lessThan(menu.group));
      expect(label.tip, isNull);
      expect(label.regions(), hasLength(1));
      expect(label.regions().first['radius'], 8);
      expect(label.size.height, labelHeight);
      expect(labelHeight, 24);
      expect(menu.regions().first['radius'], 12);
      // A label's glass profile (#168), the menu's own.
      expect(label.regions().map((r) => r['profile']), everyElement('tooltip'));
      expect(menu.regions().first['profile'], 'menu');

      // No background of its own: only the shadow image (see CachedShadow).
      final painters = tester
          .widgetList<CustomPaint>(
            find.descendant(
              of: find.byType(DockLabel),
              matching: find.byType(CustomPaint),
            ),
          )
          .map((p) => p.painter?.runtimeType.toString());
      expect(painters, contains('_ShadowImagePainter'));
      expect(
        painters.where((p) => p != '_ShadowImagePainter' && p != 'null'),
        isEmpty,
      );
    });
  }

  test('the label width follows the kit: text on the 4 pt grid + 16', () {
    expect(dockLabelWidth('', const TextStyle()), 16);
  });
}
