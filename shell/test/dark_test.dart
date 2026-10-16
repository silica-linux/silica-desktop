// The dark appearance of the shell components: with a dark `platformBrightness` the dark
// colors (measured on the references or estimated, see the comments of the color classes).

import 'dart:io';

import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:silica_shell/dock/dock.dart';
import 'package:silica_shell/dock/model.dart';
import 'package:silica_shell/dock/stack.dart';
import 'package:silica_shell/dock/stack_settings.dart';
import 'package:silica_shell/faro/launcher.dart';
import 'package:silica_shell/forcequit/force_quit.dart';
import 'package:silica_shell/menubar/window_menu.dart';
import 'package:silica_shell/screenshot/screenshot.dart';
import 'package:silica_shell/switcher/switcher.dart';
import 'package:silica_shell/system/desktop_entries.dart';
import 'package:silica_shell/system/shell_channel.dart';
import 'package:silica_ui/silica_ui.dart';

Widget dark(Widget child) => MediaQuery(
  data: const MediaQueryData(platformBrightness: Brightness.dark),
  child: Directionality(textDirection: TextDirection.ltr, child: child),
);

void main() {
  setUp(() {
    TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
      ..setMockMethodCallHandler(GlassRegistry.channel, (call) async => true)
      ..setMockMethodCallHandler(
        const MethodChannel('silica/layer', JSONMethodCodec()),
        (call) async => true,
      );
  });

  Future<void> send(WidgetTester tester, String method) =>
      tester.binding.defaultBinaryMessenger.handlePlatformMessage(
        'silica/shell',
        const JSONMethodCodec().encodeMethodCall(MethodCall('command', method)),
        (_) {},
      );

  testWidgets('switcher: #a9aaaf box and #f7f7f8 label', (tester) async {
    tester.view.physicalSize = const Size(2560, 1440);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.reset);
    final apps = [
      for (var i = 0; i < 3; i++)
        SwitcherApp(key: '$i', name: 'App $i', windows: ['$i']),
    ];
    await tester.pumpWidget(dark(SwitcherPanel(apps: apps, selected: 1)));
    final boxes = tester.widgetList<DecoratedBox>(find.byType(DecoratedBox));
    expect(
      boxes.any(
        (b) => (b.decoration as BoxDecoration).color == selectionColorDark,
      ),
      isTrue,
    );
    expect(
      tester.widget<Text>(find.text('App 1')).style?.color,
      labelColorDark,
    );
  });

  testWidgets('launcher: #edeef2 placeholder, translucent white capsules', (
    tester,
  ) async {
    const entries = {
      'calc': DesktopEntry(
        id: 'calc',
        name: 'Calcolatrice',
        exec: 'calc',
        categories: ['Utility'],
      ),
    };
    await tester.pumpWidget(
      dark(
        Align(
          alignment: Alignment.topLeft,
          child: Launcher(
            entries: entries,
            recents: const [],
            onLaunch: (_) {},
            onBack: () {},
          ),
        ),
      ),
    );
    expect(
      tester.widget<Text>(find.text('Applications')).style?.color,
      const Color(0xFFEDEEF2),
    );
    expect(
      tester.widget<Text>(find.text('Utilities')).style?.color,
      FaroColors.dark.categoryText,
    );
    expect(
      tester.widget<Text>(find.text('Calcolatrice')).style?.color,
      FaroColors.dark.text,
    );
    // The panel shadow is denser in dark.
    expect(
      FaroColors.dark.shadow(const Color(0x40000000)).a,
      closeTo(0.25 * 1.7, 0.01),
    );
  });

  testWidgets('Dock: dark separator, indicator and label', (tester) async {
    final context = await _context(tester);
    expect(DockColors.of(context).separator, const Color(0x29F4F5F7));
    expect(DockColors.of(context).indicator, const Color(0xFFB0B4BC));
    expect(MenuColors.of(context).text, const Color(0xD9FFFFFF));
  });

  testWidgets('grid stack: light title and names', (tester) async {
    final dir = Directory.systemTemp.createTempSync('griglia-scura');
    addTearDown(() => dir.deleteSync(recursive: true));
    for (var i = 1; i <= 12; i++) {
      File('${dir.path}/doc-$i.txt').writeAsStringSync('x');
    }
    tester.view.physicalSize = const Size(2560, 1440);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.reset);
    final items = [Item.app('Download', command: 'x', folder: dir.path)];
    await tester.pumpWidget(
      dark(
        Dock(
          items: items,
          onOpenPath: (_) {},
          stackSettingsFor: (_) =>
              const StackSettings(content: StackContent.grid),
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
    final name = find.text('doc-1.txt');
    expect(name, findsOneWidget);
    expect(
      DefaultTextStyle.of(tester.element(name)).style.color,
      AmalfiColors.of(Brightness.dark).textPrimary,
    );
    final title = find.text('Download').last;
    expect(tester.widget<Text>(title).style?.color, AmalfiColors.of(Brightness.dark).textPrimary);
  });

  testWidgets('window popover: #e1e1e1 glyphs', (tester) async {
    await tester.pumpWidget(
      dark(
        Align(
          alignment: Alignment.topLeft,
          child: MenuPanel(entries: windowMenuEntries((_) {})),
        ),
      ),
    );
    final painters = tester
        .widgetList<CustomPaint>(find.byType(CustomPaint))
        .map((p) => p.painter)
        .whereType<TilePainter>()
        .toList();
    expect(painters, hasLength(9));
    // The eight icons of the rows and the full screen item in the text color.
    expect(painters.where((p) => p.color == tileGlyphDark), hasLength(8));
  });

  testWidgets('force quit: dark panel', (tester) async {
    tester.view.physicalSize = const Size(1280, 800);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.reset);
    tester.binding.defaultBinaryMessenger.setMockMethodCallHandler(
      ShellChannel.channel,
      (call) async => switch (call.method) {
        'list' => [
          {'id': 'w1', 'title': 'Nota', 'app_id': 'org.prova.Nota'},
        ],
        'focused' => 'w1',
        _ => null,
      },
    );
    await ShellChannel.instance.refresh();
    await tester.pumpWidget(dark(const ForceQuitLayer(entries: {})));
    await send(tester, 'app.force_quit_panel');
    await tester.pump();
    final panel = tester.widget<Container>(
      find.byKey(const ValueKey('force quit')),
    );
    // Measured (K-06): background #272727, chosen row #0059d1, button #007aff, names #dedede.
    expect((panel.decoration! as BoxDecoration).color, const Color(0xFF272727));
    expect(
      tester
          .widget<Container>(
            find
                .descendant(
                  of: find.byKey(const ValueKey('app org.prova.Nota')),
                  matching: find.byType(Container),
                )
                .first,
          )
          .color,
      const Color(0xFF0059D1),
    );
    final button = tester.widget<Container>(
      find
          .descendant(
            of: find.byKey(const ValueKey('force quit button')),
            matching: find.byType(Container),
          )
          .first,
    );
    expect(
      (button.decoration! as BoxDecoration).color,
      const Color(0xFF007AFF),
    );
    expect(
      tester.widget<Text>(find.textContaining('does not respond')).style?.color,
      const Color(0xFFDEDEDE),
    );
    expect(
      tester.widget<Text>(find.text('Quit apps')).style?.color,
      ForceQuitColors.dark.title,
    );
  });

  testWidgets('screenshot bar: light text', (tester) async {
    tester.view.physicalSize = const Size(1280, 800);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.reset);
    tester.binding.defaultBinaryMessenger.setMockMethodCallHandler(
      ShellChannel.channel,
      (call) async => true,
    );
    final dir = Directory.systemTemp.createTempSync('shot-dark');
    addTearDown(() => dir.deleteSync(recursive: true));
    await tester.pumpWidget(
      dark(ScreenshotLayer(settingsFile: File('${dir.path}/screenshot.json'))),
    );
    await send(tester, 'screenshot.toolbar');
    await tester.pump();
    // Measured over grey (K-06): glyphs and "Options" #c7c8cd, chosen glyph #dbdde2.
    expect(
      tester.widget<Text>(find.textContaining('Options')).style?.color,
      const Color(0xFFC7C8CD),
    );
    expect(ShotBarColors.dark.chosenGlyph, const Color(0xFFDBDDE2));
    final chosen = tester.widget<Container>(
      find
          .descendant(
            of: find.byKey(const ValueKey('mode selection')),
            matching: find.byType(Container),
          )
          .first,
    );
    expect(
      (chosen.decoration! as BoxDecoration).color,
      ShotBarColors.dark.chosen,
    );
  });
}

/// A context with the dark appearance.
Future<BuildContext> _context(WidgetTester tester) async {
  late BuildContext context;
  await tester.pumpWidget(
    dark(
      Builder(
        builder: (c) {
          context = c;
          return const SizedBox();
        },
      ),
    ),
  );
  return context;
}
