import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:silica_shell/faro/faro.dart';
import 'package:silica_shell/faro/folders.dart';
import 'package:silica_shell/system/desktop_entries.dart';
import 'package:silica_shell/system/open_app.dart';
import 'package:silica_shell/system/shell_channel.dart';

const entries = {
  'calc': DesktopEntry(id: 'calc', name: 'Calculator', exec: 'calc'),
  'clock': DesktopEntry(id: 'clock', name: 'Clock', exec: 'clock'),
  'term': DesktopEntry(id: 'term', name: 'Terminal', exec: 'terminale'),
};

/// Records the silica/shell calls (launch, activate); the layer answers setTransient.
List<MethodCall> mockShell(WidgetTester tester) {
  final calls = <MethodCall>[];
  final messenger = tester.binding.defaultBinaryMessenger;
  messenger.setMockMethodCallHandler(
    const MethodChannel('silica/shell', JSONMethodCodec()),
    (call) async {
      calls.add(call);
      return true;
    },
  );
  messenger.setMockMethodCallHandler(
    const MethodChannel('silica/layer', JSONMethodCodec()),
    (call) async => call.method == 'setTransient' ? true : null,
  );
  addTearDown(() {
    messenger.setMockMethodCallHandler(
      const MethodChannel('silica/shell', JSONMethodCodec()),
      null,
    );
    messenger.setMockMethodCallHandler(
      const MethodChannel('silica/layer', JSONMethodCodec()),
      null,
    );
    ShellChannel.instance.toplevels.value = const [];
    ShellChannel.instance.minimized.value = const [];
    ShellChannel.instance.focused.value = '';
  });
  return calls;
}

Future<void> pump(WidgetTester tester, {bool launcher = false}) async {
  tester.view.physicalSize = const Size(1440, 900);
  tester.view.devicePixelRatio = 1;
  addTearDown(tester.view.reset);
  await tester.pumpWidget(
    Directionality(
      textDirection: TextDirection.ltr,
      child: Faro(
        key: UniqueKey(),
        entries: entries,
        startOpen: !launcher,
        startLauncher: launcher,
        folders: FolderIndex.empty,
      ),
    ),
  );
  await tester.pumpAndSettle();
}

Iterable<Object?> argsOf(List<MethodCall> calls, String method, String key) =>
    calls
        .where((c) => c.method == method)
        .map((c) => (c.arguments as Map)[key]);

void main() {
  testWidgets('Faro: a click on every row opens a stopped app', (tester) async {
    for (final name in ['Calculator', 'Clock', 'Terminal']) {
      final calls = mockShell(tester);
      await pump(tester);
      await tester.enterText(find.byType(EditableText), 'l');
      await tester.pumpAndSettle();
      await tester.tap(find.text(name, findRichText: true).first);
      await tester.pumpAndSettle();
      final exec = entries.values.firstWhere((e) => e.name == name).exec;
      expect(argsOf(calls, 'launch', 'command'), [exec], reason: name);
      expect(find.byType(EditableText), findsNothing, reason: name);
    }
  });

  testWidgets('Faro: a click on a running app activates it, nothing launches', (
    tester,
  ) async {
    final calls = mockShell(tester);
    ShellChannel.instance.toplevels.value = const [
      Toplevel(id: '7', title: 'Terminal', appId: 'terminale'),
      Toplevel(id: '9', title: 'Calc', appId: 'calc'),
    ];
    ShellChannel.instance.focused.value = '9';
    await pump(tester);
    await tester.enterText(find.byType(EditableText), 'l');
    await tester.pumpAndSettle();
    await tester.tap(find.text('Terminal', findRichText: true).first);
    await tester.pumpAndSettle();
    expect(argsOf(calls, 'launch', 'command'), isEmpty);
    expect(argsOf(calls, 'activate', 'id'), ['7']);
  });

  testWidgets('Launcher: a click on an app icon opens it, running or not', (
    tester,
  ) async {
    final calls = mockShell(tester);
    ShellChannel.instance.toplevels.value = const [
      Toplevel(id: '7', title: 'Terminal', appId: 'terminale'),
    ];
    await pump(tester, launcher: true);
    await tester.tap(find.text('Clock').first);
    await tester.pumpAndSettle();
    expect(argsOf(calls, 'launch', 'command'), ['clock']);

    await pump(tester, launcher: true);
    await tester.tap(find.text('Terminal', findRichText: true).first);
    await tester.pumpAndSettle();
    expect(argsOf(calls, 'launch', 'command'), ['clock']);
    expect(argsOf(calls, 'activate', 'id'), ['7']);
  });

  testWidgets(
    'Launcher: a click on the icon or the gap of a cell opens the app',
    (tester) async {
      final calls = mockShell(tester);
      await pump(tester, launcher: true);
      final label = tester.getCenter(find.text('Terminal').first);
      // The icon sits above the label; the cell's own corner is empty.
      await tester.tapAt(label - const Offset(0, 30));
      await tester.pumpAndSettle();
      expect(argsOf(calls, 'launch', 'command'), ['terminale']);
      expect(find.text('Applications'), findsNothing);
    },
  );

  Future<void> command(WidgetTester tester, String id, [int ms = 0]) async {
    await tester.binding.defaultBinaryMessenger.handlePlatformMessage(
      'silica/shell',
      const JSONMethodCodec().encodeMethodCall(MethodCall('command', id)),
      (_) {},
    );
    if (ms == 0) {
      await tester.pumpAndSettle();
    } else {
      await tester.pump(Duration(milliseconds: ms));
    }
  }

  Future<void> pumpClosed(WidgetTester tester) async {
    tester.view.physicalSize = const Size(1440, 900);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.reset);
    await tester.pumpWidget(
      Directionality(
        textDirection: TextDirection.ltr,
        child: Faro(entries: entries, folders: FolderIndex.empty),
      ),
    );
    await tester.pumpAndSettle();
  }

  testWidgets('launcher.toggle opens the Launcher at once and every time', (
    tester,
  ) async {
    mockShell(tester);
    await pumpClosed(tester);
    for (var round = 0; round < 3; round++) {
      await command(tester, 'launcher.toggle');
      expect(find.text('Applications'), findsOneWidget, reason: 'open $round');
      await command(tester, 'launcher.toggle');
      expect(find.text('Applications'), findsNothing, reason: 'close $round');
    }
  });

  testWidgets('launcher.toggle right after faro.toggle closed Faro opens it', (
    tester,
  ) async {
    mockShell(tester);
    await pumpClosed(tester);
    await command(tester, 'faro.toggle');
    expect(find.text('Search with Faro'), findsOneWidget);
    await command(tester, 'faro.toggle', 10);
    await command(tester, 'launcher.toggle');
    expect(find.text('Applications'), findsOneWidget);
    // And the other way: Launcher closed, then Faro and the Launcher at once.
    await command(tester, 'launcher.toggle', 10);
    await command(tester, 'launcher.toggle');
    expect(find.text('Applications'), findsOneWidget);
  });

  testWidgets('the input region covers every grown result row and grid cell', (
    tester,
  ) async {
    final layer = <MethodCall>[];
    final messenger = tester.binding.defaultBinaryMessenger;
    messenger.setMockMethodCallHandler(
      const MethodChannel('silica/layer', JSONMethodCodec()),
      (call) async {
        layer.add(call);
        return call.method == 'setTransient' ? true : null;
      },
    );
    addTearDown(
      () => messenger.setMockMethodCallHandler(
        const MethodChannel('silica/layer', JSONMethodCodec()),
        null,
      ),
    );
    Rect region() {
      final r =
          ((layer.lastWhere((c) => c.method == 'setInputRegion').arguments
                          as Map)['rects']
                      as List)
                  .first
              as List;
      return Rect.fromLTWH(
        (r[0] as num).toDouble(),
        (r[1] as num).toDouble(),
        (r[2] as num).toDouble(),
        (r[3] as num).toDouble(),
      );
    }

    await pump(tester);
    await tester.enterText(find.byType(EditableText), 'l');
    await tester.pumpAndSettle();
    for (final name in ['Calculator', 'Clock', 'Terminal']) {
      final at = tester.getCenter(find.text(name, findRichText: true).first);
      expect(
        region().contains(at),
        isTrue,
        reason: '$name at $at in ${region()}',
      );
    }

    // Faro to the Launcher (the Apps command): the region becomes the grid panel.
    await tester.binding.defaultBinaryMessenger.handlePlatformMessage(
      'silica/shell',
      const JSONMethodCodec().encodeMethodCall(
        const MethodCall('command', 'launcher.toggle'),
      ),
      (_) {},
    );
    await tester.pumpAndSettle();
    for (final name in ['Calculator', 'Clock', 'Terminal']) {
      final at = tester.getCenter(find.text(name, findRichText: true).first);
      expect(
        region().contains(at),
        isTrue,
        reason: '$name at $at in ${region()}',
      );
    }
  });

  test(
    'windowsToRaise: the focused window ends on top; minimized come back',
    () {
      const term = DesktopEntry(
        id: 'term',
        name: 'Terminal',
        exec: 'terminale',
      );
      const windows = [
        Toplevel(id: '1', title: 'a', appId: 'terminale'),
        Toplevel(id: '2', title: 'b', appId: 'terminale'),
        Toplevel(id: '3', title: 'c', appId: 'other'),
      ];
      expect(
        windowsToRaise(term, toplevels: windows, minimized: [], focused: '1'),
        ['2', '1'],
      );
      expect(
        windowsToRaise(
          term,
          toplevels: windows,
          minimized: ['1', '2'],
          focused: '',
        ),
        ['2'],
      );
      expect(
        windowsToRaise(term, toplevels: const [], minimized: [], focused: ''),
        isEmpty,
      );
    },
  );
}
