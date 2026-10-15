import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:silica_shell/dock/dock.dart';
import 'package:silica_ui/silica_ui.dart';

// The Dock label (TIP-01) is glass drawn by Ossido like the menus: same material, same
// shadow colour, no background of its own, light and dark.
void main() {
  setUp(() {
    TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
        .setMockMethodCallHandler(GlassRegistry.channel, (call) async => true);
  });

  for (final brightness in Brightness.values) {
    testWidgets('label declares the menu glass (${brightness.name})', (
      tester,
    ) async {
      await tester.pumpWidget(
        MediaQuery(
          data: MediaQueryData(platformBrightness: brightness),
          child: Directionality(
            textDirection: TextDirection.ltr,
            child: Column(
              children: [
                const SizedBox(
                  width: 120,
                  height: 40,
                  child: DockLabel(text: 'Files', style: TextStyle()),
                ),
                SizedBox(
                  width: 200,
                  height: 80,
                  child: MenuPanel(
                    entries: [MenuAction('Open', onSelected: () {})],
                    tipX: 100,
                  ),
                ),
              ],
            ),
          ),
        ),
      );
      final glasses = tester
          .renderObjectList<RenderGlass>(
            find.byWidgetPredicate((w) => w is Glass),
          )
          .toList();
      expect(glasses, hasLength(2));
      final label = glasses[0];
      final menu = glasses[1];
      expect(label.material, GlassMaterial.thick);
      expect(label.material, menu.material);
      expect(label.group, menu.group);
      expect(label.regions().first['opacity'], menu.regions().first['opacity']);
      expect(label.tip, isNotNull);
      expect(label.regions(), hasLength(2));
      expect(label.regions().last['material'], menu.regions().last['material']);
      expect(label.regions().first['radius'], labelHeight / 2);
      // A label's glass profile (#168); the menu takes the material's default.
      expect(label.regions().map((r) => r['profile']), everyElement('tooltip'));
      expect(menu.regions().first.containsKey('profile'), isFalse);

      // No background of its own: neither the gradient nor the border painter (the shadow is an
      // image, see CachedShadow).
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
}
