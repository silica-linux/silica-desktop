import 'package:flutter/rendering.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:silica_shell/dock/dock.dart';
import 'package:silica_shell/faro/faro.dart';
import 'package:silica_shell/notifications/model.dart';
import 'package:silica_shell/notifications/notifications.dart';
import 'package:silica_shell/system/shadow_image.dart';
import 'package:silica_ui/silica_ui.dart';

// Impeller cannot fade a blurred draw under a clip through an Opacity ancestor: the shadows
// are drawn as images (CachedShadow), and the image takes the opacity.

const _boundary = ValueKey('boundary');

Widget _under(double opacity, Widget child) => Directionality(
  textDirection: TextDirection.ltr,
  child: Center(
    child: RepaintBoundary(
      key: _boundary,
      child: ColoredBox(
        color: const Color(0xFFFFFFFF),
        child: SizedBox(
          width: 300,
          height: 200,
          child: Center(
            child: Opacity(opacity: opacity, child: child),
          ),
        ),
      ),
    ),
  ),
);

/// Darkness of the black shadow at `point` of the boundary (white background: 255 - red).
Future<int> _darkness(WidgetTester tester, Offset point) async {
  final boundary =
      tester.renderObject(find.byKey(_boundary)) as RenderRepaintBoundary;
  return (await tester.runAsync(() async {
    final image = await boundary.toImage();
    final data = (await image.toByteData())!;
    final i = (point.dy.round() * image.width + point.dx.round()) * 4;
    return 255 - data.getUint8(i);
  }))!;
}

Widget _outer() => const OuterShadow(
  radius: 10,
  blurRadius: 20,
  offset: Offset(0, 4),
  color: Color(0xFF000000),
  child: SizedBox(width: 100, height: 60),
);

void main() {
  setUp(() {
    TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
        .setMockMethodCallHandler(GlassRegistry.channel, (call) async => true);
  });

  testWidgets('a cached shadow is drawn as an image, not as a blur', (
    tester,
  ) async {
    await tester.pumpWidget(_under(1, _outer()));
    final shadow = find.byType(CachedShadow);
    expect(shadow, findsOneWidget);
    expect(tester.state<CachedShadowState>(shadow).hasImage, isTrue);
    final render = tester.renderObject(
      find.descendant(of: shadow, matching: find.byType(CustomPaint)).first,
    );
    expect(render, paints..drawImageRect());
    expect(render, isNot(paints..rrect()));
  });

  testWidgets('the shadow of a panel follows the opacity of its ancestor', (
    tester,
  ) async {
    // Just below the panel (centered in 300 x 200: x 100-200, y 70-130), in the shadow.
    const below = Offset(150, 140);
    await tester.pumpWidget(_under(1, _outer()));
    final full = await _darkness(tester, below);
    expect(full, greaterThan(40));

    await tester.pumpWidget(_under(0.5, _outer()));
    final half = await _darkness(tester, below);
    expect(half, closeTo(full / 2, 3));

    await tester.pumpWidget(_under(0, _outer()));
    expect(await _darkness(tester, below), 0);
  });

  testWidgets('the image is kept while the layout is the same', (tester) async {
    await tester.pumpWidget(_under(1, _outer()));
    final state = tester.state<CachedShadowState>(find.byType(CachedShadow));
    await tester.pumpWidget(_under(0.7, _outer()));
    expect(
      tester.state<CachedShadowState>(find.byType(CachedShadow)),
      same(state),
    );
    expect(state.hasImage, isTrue);
  });

  testWidgets('its own opacity fades the image too', (tester) async {
    Widget shadow(double opacity) => _under(
      1,
      CachedShadow(
        cacheKey: 'x',
        opacity: opacity,
        paintShadow: (canvas, size) => canvas.drawRect(
          Offset.zero & size,
          Paint()
            ..color = const Color(0xFF000000)
            ..maskFilter = const MaskFilter.blur(BlurStyle.normal, 2),
        ),
        child: const SizedBox(width: 100, height: 60),
      ),
    );
    await tester.pumpWidget(shadow(1));
    final full = await _darkness(tester, const Offset(150, 100));
    await tester.pumpWidget(shadow(0.25));
    expect(
      await _darkness(tester, const Offset(150, 100)),
      closeTo(full / 4, 3),
    );
  });

  testWidgets('the Dock label shadow is an image', (tester) async {
    await tester.pumpWidget(
      _under(
        0.5,
        const SizedBox(
          width: 120,
          height: 40,
          child: DockLabel(text: 'Files', style: TextStyle()),
        ),
      ),
    );
    final shadow = find.descendant(
      of: find.byType(DockLabel),
      matching: find.byType(CachedShadow),
    );
    expect(shadow, findsOneWidget);
    expect(tester.state<CachedShadowState>(shadow).hasImage, isTrue);
  });

  testWidgets('the notification card shadow is an image', (tester) async {
    await tester.pumpWidget(
      _under(
        0.5,
        SizedBox(
          width: 200,
          height: 80,
          child: NotificationCard(
            shadow: true,
            notification: ShellNotification(
              id: 1,
              appName: 'App',
              summary: 'Summary',
              timestamp: DateTime(2026, 10, 15),
            ),
          ),
        ),
      ),
    );
    final shadow = find.descendant(
      of: find.byType(NotificationCard),
      matching: find.byType(CachedShadow),
    );
    expect(shadow, findsOneWidget);
    expect(tester.state<CachedShadowState>(shadow).hasImage, isTrue);
  });
}
