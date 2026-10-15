import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:silica_shell/faro/faro.dart';
import 'package:silica_shell/faro/folders.dart';
import 'package:silica_shell/system/desktop_entries.dart';

const _entries = <String, DesktopEntry>{};

Future<Rect> _open(WidgetTester tester) async {
  await tester.pumpWidget(
    Directionality(
      textDirection: TextDirection.ltr,
      child: Faro(
        entries: _entries,
        startOpen: true,
        folders: FolderIndex.empty,
      ),
    ),
  );
  await tester.pumpAndSettle();
  return _panel(tester);
}

Rect _panel(WidgetTester tester) =>
    tester.getRect(find.byKey(const ValueKey('faro panel slot')));

void main() {
  // Logical sizes of the screen at several scales and resolutions (#161).
  const cases = [
    (Size(2560, 1440), 1.0),
    (Size(1920, 1080), 1.0),
    (Size(1536, 864), 1.25),
    (Size(1280, 720), 1.5),
    (Size(1440, 900), 2.0),
    (Size(1710, 1107), 2.0),
    (Size(1366, 768), 1.0),
  ];
  for (final (size, scale) in cases) {
    testWidgets('placed at 30 % from the top, centred: $size at ${scale}x', (
      tester,
    ) async {
      tester.view.physicalSize = size * scale;
      tester.view.devicePixelRatio = scale;
      addTearDown(tester.view.reset);
      final panel = await _open(tester);
      expect(panel.top, closeTo(size.height * 0.30, 0.5 / scale));
      expect(panel.center.dx, closeTo(size.width / 2, 0.5 / scale));
      // On a device pixel.
      expect(panel.top * scale, closeTo((panel.top * scale).round(), 1e-6));
    });
  }

  testWidgets('placed again when the scale and the size change', (
    tester,
  ) async {
    addTearDown(tester.view.reset);
    tester.view.physicalSize = const Size(1920, 1080);
    tester.view.devicePixelRatio = 1;
    final first = await _open(tester);
    expect(first.top, closeTo(324, 0.5));
    // The output goes to 2x: 1920 x 1080 physical are now 960 x 540 logical.
    tester.view.devicePixelRatio = 2;
    await tester.pumpAndSettle();
    var panel = _panel(tester);
    expect(panel.top, closeTo(162, 0.5));
    expect(panel.center.dx, closeTo(480, 0.5));
    tester.view.physicalSize = const Size(2880, 1800);
    await tester.pumpAndSettle();
    panel = _panel(tester);
    expect(panel.top, closeTo(270, 0.5));
    expect(panel.center.dx, closeTo(720, 0.5));
  });
}
