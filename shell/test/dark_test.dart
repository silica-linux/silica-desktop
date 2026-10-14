// L'aspetto scuro dei componenti della shell: con `platformBrightness` scuro i colori scuri
// (misurati sui riferimenti o stimati, vedi i commenti delle classi dei colori).

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

  testWidgets('selettore: riquadro #a9aaaf ed etichetta #f7f7f8', (
    tester,
  ) async {
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

  testWidgets('launcher: segnaposto #edeef2, capsule bianche traslucide', (
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
      tester.widget<Text>(find.text('Applicazioni')).style?.color,
      const Color(0xFFEDEEF2),
    );
    expect(
      tester.widget<Text>(find.text('Utility')).style?.color,
      FaroColors.dark.categoryText,
    );
    expect(
      tester.widget<Text>(find.text('Calcolatrice')).style?.color,
      FaroColors.dark.text,
    );
    // L'ombra dei pannelli più densa in scuro.
    expect(
      FaroColors.dark.shadow(const Color(0x40000000)).a,
      closeTo(0.25 * 1.7, 0.01),
    );
  });

  testWidgets('Dock: separatore, indicatore ed etichetta scuri', (
    tester,
  ) async {
    final context = await _context(tester);
    expect(DockColors.of(context).separator, const Color(0xFFA9AAAF));
    expect(DockColors.of(context).indicator, const Color(0xFFE5E6EB));
    expect(DockColors.of(context).labelBody, const Color(0xFF7F8086));
    expect(DockColors.of(context).labelText, const Color(0xFFF9F9F9));
  });

  testWidgets('pila a griglia: titolo e nomi chiari', (tester) async {
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
      gridLabelTextDark,
    );
    final title = find.text('Download').last;
    expect(tester.widget<Text>(title).style?.color, stackTextDark);
  });

  testWidgets('popover delle finestre: glifi #e1e1e1', (tester) async {
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
    // Le otto icone delle righe e la voce dello schermo intero nel colore del testo.
    expect(painters.where((p) => p.color == tileGlyphDark), hasLength(8));
  });

  testWidgets('uscita forzata: pannello scuro', (tester) async {
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
      find.byKey(const ValueKey('uscita forzata')),
    );
    expect(
      (panel.decoration! as BoxDecoration).color,
      ForceQuitColors.dark.fill,
    );
    expect(
      tester.widget<Text>(find.text('Termina le app')).style?.color,
      ForceQuitColors.dark.title,
    );
  });

  testWidgets('barra delle istantanee: testo chiaro', (tester) async {
    tester.view.physicalSize = const Size(1280, 800);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.reset);
    tester.binding.defaultBinaryMessenger.setMockMethodCallHandler(
      ShellChannel.channel,
      (call) async => true,
    );
    await tester.pumpWidget(dark(const ScreenshotLayer()));
    await send(tester, 'screenshot.toolbar');
    await tester.pump();
    expect(
      tester.widget<Text>(find.textContaining('Salva in:')).style?.color,
      ShotBarColors.dark.glyph,
    );
  });
}

/// Un contesto con l'aspetto scuro.
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
