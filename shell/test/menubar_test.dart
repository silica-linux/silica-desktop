import 'dart:io';

import 'package:flutter/gestures.dart' show PointerDeviceKind;
import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:silica_shell/l10n/l10n.dart';
import 'package:silica_shell/menubar/desktop_menu.dart';
import 'package:silica_shell/menubar/logo_menu.dart';
import 'package:silica_shell/menubar/menubar.dart';
import 'package:silica_shell/system/desktop_entries.dart';
import 'package:silica_shell/menubar/status_icons.dart';
import 'package:silica_shell/menubar/window_menu.dart';
import 'package:silica_ui/silica_ui.dart'
    show
        MenuAction,
        MenuCustom,
        MenuDivider,
        MenuEntry,
        MenuHost,
        MenuPanel;
import 'package:silica_shell/system/shell_channel.dart';
import 'package:silica_shell/system/system_status.dart';

import 'controlcenter_test.dart' show FakeSystem;

void main() {
  test('the clock as in the bar: day, date, double space, time', () {
    expect(clockText(DateTime(2026, 10, 12, 22, 34)), 'Mon 12 Oct  22:34');
    expect(clockText(DateTime(2026, 1, 4, 9, 5)), 'Sun 4 Jan  09:05');
    // In Italian as the bar has always shown it.
    expect(
      clockText(DateTime(2026, 10, 12, 22, 34), 'it'),
      'Lun 12 ott  22:34',
    );
    expect(clockText(DateTime(2026, 1, 4, 9, 5), 'it'), 'Dom 4 gen  09:05');
  });

  testWidgets(
    'the name of the focused app, in bold, from the .desktop entries',
    (tester) async {
      tester.binding.defaultBinaryMessenger.setMockMethodCallHandler(
        ShellChannel.channel,
        (call) async => switch (call.method) {
          'list' => [
            {'id': 'w1', 'title': 'Casa', 'app_id': 'org.gnome.Nautilus'},
          ],
          'focused' => 'w1',
          _ => null,
        },
      );
      const files = DesktopEntry(
        id: 'org.gnome.Nautilus',
        name: 'File',
        exec: 'nautilus',
      );
      await tester.pumpWidget(
        Directionality(
          textDirection: TextDirection.ltr,
          child: MenuBar(
            entries: const {'org.gnome.Nautilus': files},
            now: DateTime(2026, 10, 12, 22, 34),
          ),
        ),
      );
      await tester.pump();
      await tester.pump();
      final name = tester.widget<Text>(find.text('File').first);
      expect(name.style?.fontWeight, FontWeight.w600);
      expect(find.text('Mon 12 Oct  22:34'), findsOneWidget);
    },
  );

  test('the lit arcs follow the Wi-Fi strength', () {
    expect(wifiArcs(0.9), 3);
    expect(wifiArcs(0.5), 2);
    expect(wifiArcs(0.1), 1);
  });

  testWidgets('the Focus moon only while a Focus is on (SH-MB-26)', (
    tester,
  ) async {
    final system = FakeSystem(const SystemSnapshot(wifiAvailable: true));
    await tester.pumpWidget(
      Directionality(
        textDirection: TextDirection.ltr,
        child: MenuBar(system: system, now: DateTime(2026, 10, 13, 9)),
      ),
    );
    bool moon() => tester
        .widgetList<StatusIcon>(find.byType(StatusIcon))
        .any((i) => i.kind == StatusKind.focus);
    expect(moon(), isFalse);
    system.value = system.value.copyWith(doNotDisturb: true);
    await tester.pump();
    expect(moon(), isTrue);
  });

  testWidgets('battery and network from the system state', (tester) async {
    final system = FakeSystem(
      const SystemSnapshot(
        wifiAvailable: true,
        wifiEnabled: true,
        wifiSsid: 'Casa',
        wifiStrength: 79,
        batteryPresent: true,
        batteryPercent: 80,
        batteryCharging: true,
      ),
    );
    await tester.pumpWidget(
      Directionality(
        textDirection: TextDirection.ltr,
        child: MenuBar(system: system, now: DateTime(2026, 10, 13, 9)),
      ),
    );
    StatusIcon icon(StatusKind kind) => tester
        .widgetList<StatusIcon>(find.byType(StatusIcon))
        .firstWhere((i) => i.kind == kind);
    expect(icon(StatusKind.battery).level, 0.8);
    expect(icon(StatusKind.battery).charging, isTrue);
    expect(icon(StatusKind.network).level, 0.79);
    expect(icon(StatusKind.network).dim, isFalse);

    // Wi-Fi off: the icon stays, dimmed; wired: the wired network icon.
    system.value = system.value.copyWith(wifiEnabled: false);
    await tester.pump();
    expect(icon(StatusKind.network).dim, isTrue);
    system.value = const SystemSnapshot(wifiAvailable: true, wired: true);
    await tester.pump();
    expect(
      find.byWidgetPredicate(
        (w) => w is StatusIcon && w.kind == StatusKind.wired,
      ),
      findsOneWidget,
    );
    expect(
      find.byWidgetPredicate(
        (w) => w is StatusIcon && w.kind == StatusKind.battery,
      ),
      findsNothing,
    );
  });

  testWidgets(
    'an open menu takes the pointer only on the bar and itself; a press outside ends it (#149)',
    (tester) async {
      tester.view.physicalSize = const Size(1280, 400);
      tester.view.devicePixelRatio = 1;
      addTearDown(tester.view.reset);
      final layer = <MethodCall>[];
      // The frontmost app: the focused window's.
      tester.binding.defaultBinaryMessenger
        ..setMockMethodCallHandler(
          ShellChannel.channel,
          (call) async => switch (call.method) {
            'list' => [
              {'id': 'w1', 'title': 'Notes', 'app_id': 'dev.frascella.Note'},
            ],
            'focused' => 'w1',
            _ => null,
          },
        )
        ..setMockMethodCallHandler(
          const MethodChannel('silica/layer', JSONMethodCodec()),
          (call) async {
            layer.add(call);
            return true;
          },
        );
      addTearDown(() {
        ShellChannel.instance.toplevels.value = const [];
        ShellChannel.instance.focused.value = '';
      });
      await tester.pumpWidget(
        Directionality(
          textDirection: TextDirection.ltr,
          child: MenuBar(now: DateTime(2026, 10, 12, 22, 34)),
        ),
      );
      await tester.pump();
      await tester.tapAt(tester.getRect(find.text('Window')).center);
      await tester.pump();
      await tester.pump();
      final transient = layer.lastWhere((c) => c.method == 'setTransient');
      // A menu of the frontmost app (INPUT-01 (b)): modal for that app, a click in
      // another app closes it and goes through.
      final args = transient.arguments as Map;
      expect(args['owner'], 'dev.frascella.Note');
      expect(args['ownerRule'], 'modal');
      expect(args['otherRule'], 'pass');
      // An older Ossido (version 6): consumed outside, as before.
      expect(args['press'], 'consume');
      final region = layer.lastWhere((c) => c.method == 'setInputRegion');
      final rects = (region.arguments as Map)['rects'] as List;
      expect(rects.length, 2); // the bar and the menu, not the whole screen

      await tester.binding.defaultBinaryMessenger.handlePlatformMessage(
        'silica/shell',
        const JSONMethodCodec().encodeMethodCall(
          const MethodCall('transientDismissed'),
        ),
        (_) {},
      );
      await tester.pumpAndSettle();
      expect(find.text('Minimize'), findsNothing);
      expect(layer.last.method, 'setSize');
    },
  );

  testWidgets(
    'a click that passes to another app closes the menu even as the frontmost app changes (#149)',
    (tester) async {
      tester.view.physicalSize = const Size(1280, 400);
      tester.view.devicePixelRatio = 1;
      addTearDown(tester.view.reset);
      final layer = <MethodCall>[];
      tester.binding.defaultBinaryMessenger
        ..setMockMethodCallHandler(
          ShellChannel.channel,
          (call) async => switch (call.method) {
            'list' => [
              {'id': 'w1', 'title': 'Notes', 'app_id': 'dev.frascella.Note'},
            ],
            'focused' => 'w1',
            _ => null,
          },
        )
        ..setMockMethodCallHandler(
          const MethodChannel('silica/layer', JSONMethodCodec()),
          (call) async {
            layer.add(call);
            return true;
          },
        );
      addTearDown(() {
        ShellChannel.instance.toplevels.value = const [];
        ShellChannel.instance.focused.value = '';
        ShellChannel.instance.states.value = const {};
      });
      await tester.pumpWidget(
        Directionality(
          textDirection: TextDirection.ltr,
          child: MenuBar(now: DateTime(2026, 10, 12, 22, 34)),
        ),
      );
      await tester.pump();
      await tester.tapAt(tester.getRect(find.text('Window')).center);
      await tester.pump();
      await tester.pump();
      expect(find.text('Minimize'), findsOneWidget);

      // Ossido ends the menu, then the same click makes the desktop's owner frontmost:
      // the bar rebuilds for the new app while the menu fades out.
      Future<void> shell(MethodCall call) =>
          tester.binding.defaultBinaryMessenger.handlePlatformMessage(
            'silica/shell',
            const JSONMethodCodec().encodeMethodCall(call),
            (_) {},
          );
      await shell(const MethodCall('transientDismissed'));
      await tester.pump(const Duration(milliseconds: 16));
      await shell(const MethodCall('focused', ''));
      await shell(
        const MethodCall('broadcast', {
          'topic': 'app.frontmost',
          'value': 'dev.frascella.Mustache',
        }),
      );
      await tester.pumpAndSettle();
      expect(find.text('Minimize'), findsNothing);
      expect(layer.any((c) => c.method == 'clearTransient'), isTrue);
      expect(layer.last.method, 'setSize');
    },
  );

  testWidgets('pressing Window opens the menu; the choice runs the command', (
    tester,
  ) async {
    tester.view.physicalSize = const Size(1280, 400);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.reset);
    final executed = <String>[];
    final layer = <String>[];
    tester.binding.defaultBinaryMessenger
      ..setMockMethodCallHandler(ShellChannel.channel, (call) async {
        if (call.method == 'execute') {
          executed.add('${(call.arguments as Map)['id']}');
        }
        return null;
      })
      ..setMockMethodCallHandler(
        const MethodChannel('silica/layer', JSONMethodCodec()),
        (call) async {
          layer.add(call.method);
          return true;
        },
      );
    await tester.pumpWidget(
      Directionality(
        textDirection: TextDirection.ltr,
        child: MenuBar(now: DateTime(2026, 10, 12, 22, 34)),
      ),
    );
    await tester.pump();
    final finestra = tester.getRect(find.text('Window'));
    await tester.tapAt(finestra.center);
    await tester.pump();
    expect(find.text('Minimize'), findsOneWidget);
    // The surface grows and the keyboard goes to the menu.
    expect(
      layer,
      containsAll(['setSize', 'setInputRegion', 'setKeyboardInteractivity']),
    );
    // The menu body 5 pt left of the box (which has ~10.75 pt before the text).
    final riduci = tester.getTopLeft(find.text('Minimize'));
    expect(riduci.dx, closeTo(finestra.left - itemPadding - 5 + 17, 1));
    await tester.tapAt(tester.getCenter(find.text('Minimize')));
    await tester.pump(const Duration(milliseconds: 150));
    expect(executed, ['window.minimize']);
    await tester.pump(const Duration(milliseconds: 100));
    await tester.pump(const Duration(milliseconds: 200));
    expect(find.text('Minimize'), findsNothing);
  });

  testWidgets(
    'long press on the green button: the popover below the button runs the arrangement',
    (tester) async {
      tester.view.physicalSize = const Size(1280, 800);
      tester.view.devicePixelRatio = 1;
      addTearDown(tester.view.reset);
      final executed = <String>[];
      final layer = <String>[];
      tester.binding.defaultBinaryMessenger
        ..setMockMethodCallHandler(ShellChannel.channel, (call) async {
          if (call.method == 'execute') {
            executed.add('${(call.arguments as Map)['id']}');
          }
          return null;
        })
        ..setMockMethodCallHandler(
          const MethodChannel('silica/layer', JSONMethodCodec()),
          (call) async {
            layer.add(call.method);
            return call.method == 'outputSize'
                ? {'width': 1280, 'height': 800}
                : true;
          },
        );
      await tester.pumpWidget(
        Directionality(
          textDirection: TextDirection.ltr,
          child: MenuBar(now: DateTime(2026, 10, 12, 22, 34)),
        ),
      );
      await tester.pump();
      await tester.binding.defaultBinaryMessenger.handlePlatformMessage(
        'silica/shell',
        const JSONMethodCodec().encodeMethodCall(
          MethodCall('windowMenu', {'id': 'w1', 'x': 300, 'y': 200}),
        ),
        (_) {},
      );
      await tester.pump();
      await tester.pump();
      expect(find.text('Half of the screen'), findsOneWidget);
      // The body below the tip: 8 pt below the apex, 27 to the left.
      final panel = tester.getRect(find.byType(MenuPanel));
      expect(panel.top, 208);
      expect(panel.left, 273);
      expect(panel.width, windowMenuWidth);
      await tester.tap(find.byKey(const ValueKey('window.tile_left')));
      // Blink and fade of the closing (~380 ms).
      for (var i = 0; i < 12; i++) {
        await tester.pump(const Duration(milliseconds: 50));
      }
      expect(executed, ['window.tile_left']);
      expect(find.text('Half of the screen'), findsNothing);
    },
  );

  testWidgets(
    'capsules on the controls icon and on the clock with the Centers open',
    (tester) async {
      tester.view.physicalSize = const Size(1280, 400);
      tester.view.devicePixelRatio = 1;
      addTearDown(tester.view.reset);
      tester.binding.defaultBinaryMessenger.setMockMethodCallHandler(
        ShellChannel.channel,
        (call) async => null,
      );
      await tester.pumpWidget(
        Directionality(
          textDirection: TextDirection.ltr,
          child: MenuBar(now: DateTime(2026, 10, 12, 22, 34)),
        ),
      );
      await tester.pump();
      Future<void> announce(String topic, String value) =>
          tester.binding.defaultBinaryMessenger.handlePlatformMessage(
            'silica/shell',
            const JSONMethodCodec().encodeMethodCall(
              MethodCall('broadcast', {'topic': topic, 'value': value}),
            ),
            (_) {},
          );
      const controls = ValueKey('control center capsule');
      const clock = ValueKey('clock capsule');
      expect(find.byKey(controls), findsNothing);
      await announce('controlcenter.open', '1');
      await tester.pump();
      await tester.pump();
      final capsule = tester.getRect(find.byKey(controls));
      expect(capsule.size, controlsCapsule);
      await announce('notifications.open', '1');
      await tester.pump();
      expect(find.byKey(clock), findsOneWidget);
      await announce('controlcenter.open', '0');
      await announce('notifications.open', '0');
      await tester.pump();
      expect(find.byKey(controls), findsNothing);
      expect(find.byKey(clock), findsNothing);
    },
  );

  group('logo menu', () {
    List<String> labels(List<MenuEntry> entries) => [
      for (final e in entries)
        switch (e) {
          MenuAction(:final label) => label,
          MenuDivider() => '—',
          _ => '?',
        },
    ];

    List<MenuEntry> menu({bool option = false}) => logoMenu(
      l: englishTexts,
      run: (_) {},
      launch: (_) {},
      open: (_) {},
      option: option,
      user: 'Ada Lovelace',
    );

    test('items in the reference order, power items without the session', () {
      expect(labels(menu()), [
        'About Silica',
        '—',
        'Settings…',
        '—',
        'Recent Items',
        '—',
        'Force Quit…',
        '—',
        'Sleep',
        'Restart…',
        'Shut Down…',
        '—',
        'Lock Screen',
        'Log Out Ada Lovelace…',
      ]);
      final logOut = menu().last as MenuAction;
      expect(logOut.shortcut, '⇧⌘Q');
      expect(logOut.enabled, isTrue);
      expect((menu()[8] as MenuAction).enabled, isFalse);
    });

    test('with Option held: the alternate items, same number of rows', () {
      final alt = labels(menu(option: true));
      expect(alt.first, 'System Information');
      expect(alt, contains('Restart'));
      expect(alt, contains('Shut Down'));
      expect(alt.last, 'Log Out Ada Lovelace');
      expect((menu(option: true).last as MenuAction).shortcut, '⌥⇧⌘Q');
      expect(alt.length, labels(menu()).length);
    });

    test('recent documents from the xbel list, newest first', () {
      const xml = """
<xbel version="1.0">
  <bookmark href="file:///home/u/old.txt" added="x" modified="2026-10-01T10:00:00Z" visited="x"/>
  <bookmark href="file:///home/u/new%20one.pdf" modified="2026-10-14T09:00:00Z"/>
  <bookmark href="https://example.org/" modified="2026-10-14T10:00:00Z"/>
</xbel>""";
      expect(parseRecentlyUsed(xml), [
        '/home/u/new one.pdf',
        '/home/u/old.txt',
      ]);
      expect(parseRecentlyUsed(xml, after: '2026-10-10T00:00:00Z'), [
        '/home/u/new one.pdf',
      ]);
    });

    test('the full name from passwd, else the login', () {
      const passwd =
          'root:x:0:0:root:/root:/bin/bash\n'
          'ada:x:1000:1000:Ada Lovelace,,,:/home/ada:/bin/bash\n'
          'bob:x:1001:1001::/home/bob:/bin/sh\n';
      expect(
        userFullName(env: {'USER': 'ada'}, passwd: passwd),
        'Ada Lovelace',
      );
      expect(userFullName(env: {'USER': 'bob'}, passwd: passwd), 'bob');
    });
  });

  group('desktop menu', () {
    test('the desktop folder from user-dirs, else ~/Desktop', () {
      expect(
        desktopFolder(
          env: {'HOME': '/home/u'},
          userDirs: 'XDG_DESKTOP_DIR="\$HOME/Scrivania"\n',
        ),
        '/home/u/Scrivania',
      );
      expect(
        desktopFolder(env: {'HOME': '/home/u'}, userDirs: ''),
        '/home/u/Desktop',
      );
    });

    test('New Folder picks a free name', () {
      final dir = Directory.systemTemp.createTempSync('desktop');
      addTearDown(() => dir.deleteSync(recursive: true));
      expect(
        newFolder(dir.path, 'untitled folder'),
        '${dir.path}/untitled folder',
      );
      expect(
        newFolder(dir.path, 'untitled folder'),
        '${dir.path}/untitled folder 2',
      );
    });

    test('the items of the reference, the unavailable ones disabled', () {
      final entries = desktopMenu(l: englishTexts, onNewFolder: () {});
      final actions = entries.whereType<MenuAction>().toList();
      expect(
        [for (final a in actions) a.label],
        [
          'New Folder',
          'Paste Item',
          'Get Info',
          'Change Wallpaper…',
          'Edit Widgets…',
          'Use Stacks',
          'Sort By',
          'Show View Options',
        ],
      );
      expect(actions.first.enabled, isTrue);
      expect(actions[3].enabled, isFalse);
      expect(entries.whereType<MenuDivider>(), hasLength(2));
    });

    test(
      'who asked for the menu: the wallpaper or the file manager (#147)',
      () {
        expect(parseDesktopMenu('10 20'), (
          const Offset(10, 20),
          DesktopMenuKind.wallpaper,
        ));
        expect(parseDesktopMenu('10 20 desktop')?.$2, DesktopMenuKind.desktop);
        expect(parseDesktopMenu('10 20 items')?.$2, DesktopMenuKind.items);
        expect(parseDesktopMenu(''), isNull);
        expect(parseDesktopMenu('10 20 other'), isNull);
        expect(parseDesktopMenu('x 20'), isNull);
      },
    );

    test('the owner state, with bad values ignored', () {
      final state = DesktopOwnerState.parse(
        '{"stacks":true,"sort":"kind","selected":2}',
      );
      expect(state.stacks, isTrue);
      expect(state.sort, 'kind');
      expect(state.selected, 2);
      final broken = DesktopOwnerState.parse('{"sort":"color","selected":-1');
      expect(broken.sort, 'none');
      expect(broken.selected, 0);
    });

    test('the file manager desktop menu sends its actions back', () {
      final sent = <String>[];
      final entries = ownerDesktopMenu(
        l: englishTexts,
        state: const DesktopOwnerState(sort: 'name'),
        onAction: sent.add,
      );
      final actions = entries.whereType<MenuAction>().toList();
      MenuAction named(String label) =>
          actions.firstWhere((a) => a.label == label);
      named('New Folder').onSelected?.call();
      named('Use Stacks').onSelected?.call();
      final sort = named('Sort By').submenu!.whereType<MenuAction>().toList();
      expect(sort.firstWhere((a) => a.label == 'Name').checked, isTrue);
      sort.firstWhere((a) => a.label == 'Size').onSelected?.call();
      // Clean Up only with free positions (Sort By › None).
      expect(named('Clean Up').enabled, isFalse);
      expect(named('Paste Item').enabled, isFalse);
      expect(sent, ['newFolder', 'stacks', 'sort:size']);
    });

    test('the owner state carries the shared colour tags and the folder', () {
      final state = DesktopOwnerState.parse(
        '{"selected":1,"tags":["red","blue","pink",3],"folder":true}',
      );
      expect(state.tags, {'red', 'blue'});
      expect(state.folder, isTrue);
      final none = DesktopOwnerState.parse('{"tags":"red"}');
      expect(none.tags, isEmpty);
      expect(none.folder, isFalse);
    });

    test('Customize Folder… replaces Tags… on one folder and sends back', () {
      final sent = <String>[];
      final entries = ownerItemsMenu(
        l: englishTexts,
        state: const DesktopOwnerState(selected: 1, folder: true),
        onAction: sent.add,
      );
      final labels = [for (final a in entries.whereType<MenuAction>()) a.label];
      expect(labels.last, 'Customize Folder…');
      expect(labels, isNot(contains('Tags…')));
      entries.whereType<MenuAction>().last.onSelected?.call();
      expect(sent, ['customize']);
      // As in the window menu: a divider, the row of dots, then the item.
      final last = entries.length - 1;
      expect(entries[last - 1], isA<MenuCustom>());
      expect(entries[last - 2], isA<MenuDivider>());
      final row = entries[last - 1] as MenuCustom;
      expect(row.height, 25);
      expect(row.width, 18 + 24.0 * 7 + 12);
    });

    group('the colour dots', () {
      Future<void> pumpRow(
        WidgetTester tester,
        Set<String> assigned,
        List<String> sent,
      ) => tester.pumpWidget(
        Directionality(
          textDirection: TextDirection.ltr,
          child: Align(
            alignment: Alignment.topLeft,
            child: DesktopTagRow(assigned: assigned, onToggle: sent.add),
          ),
        ),
      );

      testWidgets('seven dots at a 24 pt pitch, the checked ones marked', (
        tester,
      ) async {
        await pumpRow(tester, {'red', 'blue'}, []);
        const names = [
          'red',
          'orange',
          'yellow',
          'green',
          'blue',
          'purple',
          'gray',
        ];
        for (final n in names) {
          expect(find.byKey(ValueKey('tag $n')), findsOneWidget);
        }
        final first = tester.getRect(find.byKey(const ValueKey('tag red')));
        expect(first.size, const Size(24, 25));
        expect(first.left, 18 - 3);
        expect(
          tester.getRect(find.byKey(const ValueKey('tag orange'))).left -
              first.left,
          24,
        );
        // A check mark on the colours every selected icon has, none on the others.
        expect(find.text('✓'), findsNWidgets(2));
        for (final n in ['red', 'blue']) {
          expect(
            find.descendant(
              of: find.byKey(ValueKey('tag $n')),
              matching: find.text('✓'),
            ),
            findsOneWidget,
          );
        }
      });

      testWidgets('a click sends the colour back', (tester) async {
        final sent = <String>[];
        await pumpRow(tester, {'red'}, sent);
        await tester.tap(find.byKey(const ValueKey('tag green')));
        await tester.tap(find.byKey(const ValueKey('tag red')));
        expect(sent, ['green', 'red']);
      });

      testWidgets('under the pointer a dot grows and shows + or ×', (
        tester,
      ) async {
        await pumpRow(tester, {'red'}, []);
        final mouse = await tester.createGesture(kind: PointerDeviceKind.mouse);
        addTearDown(mouse.removePointer);
        await mouse.addPointer(location: Offset.zero);
        await mouse.moveTo(
          tester.getCenter(find.byKey(const ValueKey('tag green'))),
        );
        await tester.pump();
        expect(find.text('+'), findsOneWidget);
        await mouse.moveTo(
          tester.getCenter(find.byKey(const ValueKey('tag red'))),
        );
        await tester.pump();
        expect(find.text('×'), findsOneWidget);
      });
    });

    test('the menu of the selected icons', () {
      final sent = <String>[];
      final many = ownerItemsMenu(
        l: englishTexts,
        state: const DesktopOwnerState(selected: 2),
        onAction: sent.add,
      ).whereType<MenuAction>().toList();
      expect(
        [for (final a in many) a.label],
        [
          'Open',
          'Move to Trash',
          'Get Info',
          'Rename',
          'Duplicate',
          'Quick Preview',
          // Not one folder: Tags…, disabled.
          'Tags…',
        ],
      );
      expect(many.last.enabled, isFalse);
      // Rename only one at a time.
      expect(many.firstWhere((a) => a.label == 'Rename').enabled, isFalse);
      many.firstWhere((a) => a.label == 'Move to Trash').onSelected?.call();
      expect(sent, ['trash']);
    });

    testWidgets(
      'Move to Trash closes the menu while the desktop announces its new state',
      (tester) async {
        tester.view.physicalSize = const Size(1280, 800);
        tester.view.devicePixelRatio = 1;
        addTearDown(tester.view.reset);
        final layer = <MethodCall>[];
        final sent = <String>[];
        Future<void> shell(MethodCall call) =>
            tester.binding.defaultBinaryMessenger.handlePlatformMessage(
              'silica/shell',
              const JSONMethodCodec().encodeMethodCall(call),
              (_) {},
            );
        Future<void> broadcast(String topic, String value) =>
            shell(MethodCall('broadcast', {'topic': topic, 'value': value}));
        tester.binding.defaultBinaryMessenger
          ..setMockMethodCallHandler(ShellChannel.channel, (call) async {
            if (call.method == 'broadcast') {
              final args = call.arguments as Map;
              sent.add('${args['topic']}=${args['value']}');
            }
            return switch (call.method) {
              'list' => const <Object?>[],
              'focused' => '',
              _ => null,
            };
          })
          ..setMockMethodCallHandler(
            const MethodChannel('silica/layer', JSONMethodCodec()),
            (call) async {
              layer.add(call);
              return switch (call.method) {
                'outputSize' => {'width': 1280, 'height': 800},
                _ => true,
              };
            },
          );
        addTearDown(() {
          ShellChannel.instance.states.value = const {};
        });
        await tester.pumpWidget(
          Directionality(
            textDirection: TextDirection.ltr,
            child: MenuBar(now: DateTime(2026, 10, 12, 22, 34)),
          ),
        );
        await tester.pump();
        await broadcast(
          'desktop.state',
          '{"stacks":false,"sort":"none","selected":1}',
        );
        await broadcast('desktopmenu', '400 300 items');
        await tester.pump();
        await tester.pump();
        expect(find.text('Move to Trash'), findsOneWidget);

        await tester.tapAt(tester.getRect(find.text('Move to Trash')).center);
        // The desktop runs the action as the menu starts fading, and announces its new
        // state: the bar rebuilds while the menu fades out.
        for (var i = 0; i < 20; i++) {
          await tester.pump(const Duration(milliseconds: 16));
          if (sent.any((s) => s.startsWith('desktop.action=trash'))) break;
        }
        expect(sent.where((s) => s.startsWith('desktop.action=trash')), [
          'desktop.action=trash 1',
        ]);
        await broadcast(
          'desktop.state',
          '{"stacks":false,"sort":"none","selected":0}',
        );
        await tester.pumpAndSettle();
        expect(find.text('Move to Trash'), findsNothing);
        expect(sent.last, 'desktopmenu=');
        expect(layer.any((c) => c.method == 'clearTransient'), isTrue);
      },
    );

    testWidgets(
      'the item menu shows the colour dots checked from desktop.state and '
      'sends tag and customize back',
      (tester) async {
        tester.view.physicalSize = const Size(1280, 800);
        tester.view.devicePixelRatio = 1;
        addTearDown(tester.view.reset);
        final sent = <String>[];
        Future<void> broadcast(String topic, String value) =>
            tester.binding.defaultBinaryMessenger.handlePlatformMessage(
              'silica/shell',
              const JSONMethodCodec().encodeMethodCall(
                MethodCall('broadcast', {'topic': topic, 'value': value}),
              ),
              (_) {},
            );
        tester.binding.defaultBinaryMessenger
          ..setMockMethodCallHandler(ShellChannel.channel, (call) async {
            if (call.method == 'broadcast') {
              final args = call.arguments as Map;
              sent.add('${args['topic']}=${args['value']}');
            }
            return switch (call.method) {
              'list' => const <Object?>[],
              'focused' => '',
              _ => null,
            };
          })
          ..setMockMethodCallHandler(
            const MethodChannel('silica/layer', JSONMethodCodec()),
            (call) async => switch (call.method) {
              'outputSize' => {'width': 1280, 'height': 800},
              _ => true,
            },
          );
        addTearDown(() {
          ShellChannel.instance.states.value = const {};
        });
        await tester.pumpWidget(
          Directionality(
            textDirection: TextDirection.ltr,
            child: MenuBar(now: DateTime(2026, 10, 12, 22, 34)),
          ),
        );
        await tester.pump();
        await broadcast(
          'desktop.state',
          '{"stacks":false,"sort":"none","selected":1,'
              '"tags":["green"],"folder":true}',
        );
        await broadcast('desktopmenu', '400 100 items');
        await tester.pump();
        await tester.pump();
        expect(find.byKey(const ValueKey('tag red')), findsOneWidget);
        expect(find.text('✓'), findsOneWidget);
        expect(
          find.descendant(
            of: find.byKey(const ValueKey('tag green')),
            matching: find.text('✓'),
          ),
          findsOneWidget,
        );
        expect(find.text('Tags…'), findsNothing);

        await tester.tap(find.byKey(const ValueKey('tag blue')));
        for (var i = 0; i < 20; i++) {
          await tester.pump(const Duration(milliseconds: 16));
          if (sent.any((s) => s.startsWith('desktop.action=tag:blue'))) break;
        }
        expect(sent.where((s) => s.startsWith('desktop.action=')), [
          'desktop.action=tag:blue 1',
        ]);
        await tester.pumpAndSettle();

        // The menu again: Customize Folder… sends `customize`.
        await broadcast('desktopmenu', '400 100 items');
        await tester.pump();
        await tester.pump();
        await tester.tap(find.text('Customize Folder…'));
        for (var i = 0; i < 20; i++) {
          await tester.pump(const Duration(milliseconds: 16));
          if (sent.any((s) => s.startsWith('desktop.action=customize'))) break;
        }
        expect(sent.where((s) => s.startsWith('desktop.action=')), [
          'desktop.action=tag:blue 1',
          'desktop.action=customize 2',
        ]);
        await tester.pumpAndSettle();
      },
    );

    group('stays on screen', () {
      Future<Rect> openAt(WidgetTester tester, String at) async {
        tester.view.physicalSize = const Size(1280, 800);
        tester.view.devicePixelRatio = 1;
        addTearDown(tester.view.reset);
        tester.binding.defaultBinaryMessenger
          ..setMockMethodCallHandler(ShellChannel.channel, (call) async {
            return switch (call.method) {
              'list' => const <Object?>[],
              'focused' => '',
              _ => null,
            };
          })
          ..setMockMethodCallHandler(
            const MethodChannel('silica/layer', JSONMethodCodec()),
            (call) async => switch (call.method) {
              'outputSize' => {'width': 1280, 'height': 800},
              _ => true,
            },
          );
        addTearDown(() {
          ShellChannel.instance.states.value = const {};
        });
        await tester.pumpWidget(
          Directionality(
            textDirection: TextDirection.ltr,
            child: MenuBar(now: DateTime(2026, 10, 12, 22, 34)),
          ),
        );
        await tester.pump();
        await tester.binding.defaultBinaryMessenger.handlePlatformMessage(
          'silica/shell',
          const JSONMethodCodec().encodeMethodCall(
            MethodCall('broadcast', {'topic': 'desktopmenu', 'value': at}),
          ),
          (_) {},
        );
        await tester.pumpAndSettle();
        final texts = find.descendant(
          of: find.byType(MenuHost),
          matching: find.byType(Text),
        );
        expect(texts, findsWidgets);
        return [
          for (final t in texts.evaluate())
            tester.getRect(find.byWidget(t.widget)),
        ].reduce((a, b) => a.expandToInclude(b));
      }

      testWidgets('opened at the right edge it opens to the left', (
        tester,
      ) async {
        final rect = await openAt(tester, '1275 300 items');
        expect(rect.right, lessThanOrEqualTo(1275));
        expect(rect.left, greaterThanOrEqualTo(0));
      });

      testWidgets('opened near the bottom it shifts up', (tester) async {
        final rect = await openAt(tester, '400 790 items');
        expect(rect.bottom, lessThanOrEqualTo(800));
        expect(rect.top, greaterThanOrEqualTo(barHeight));
      });
    });
  });
}
