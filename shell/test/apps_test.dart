import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:silica_shell/dock/apps.dart';
import 'package:silica_shell/dock/model.dart';
import 'package:silica_shell/dock/trash_alert.dart';
import 'package:silica_shell/system/desktop_entries.dart';
import 'package:silica_shell/system/shell_channel.dart';

const String filesEntry = '''
[Desktop Entry]
Name=Files
Name[it]=File
Exec=nautilus --new-window %U
Icon=org.gnome.Nautilus
Type=Application
StartupWMClass=org.gnome.Nautilus
Keywords=folder;manager;

[Desktop Action new-window]
Name=Nuova finestra
Exec=nautilus --new-window
''';

void main() {
  test(
    'a .desktop entry: localized name, Exec without codes, actions ignored',
    () {
      final entry = parseDesktopEntry('org.gnome.Nautilus', filesEntry)!;
      expect(entry.name, 'File');
      expect(entry.exec, 'nautilus --new-window');
      expect(entry.icon, 'org.gnome.Nautilus');
      expect(entry.keywords, ['folder', 'manager']);
      expect(entry.matchesAppId('org.gnome.Nautilus'), isTrue);
      expect(entry.matchesAppId('nautilus'), isTrue);
      expect(entry.matchesAppId('firefox'), isFalse);
    },
  );

  test('hidden entries or entries without Exec are not apps', () {
    expect(
      parseDesktopEntry(
        'x',
        '[Desktop Entry]\nType=Application\nName=X\nNoDisplay=true\nExec=x',
      ),
      isNull,
    );
    expect(
      parseDesktopEntry('x', '[Desktop Entry]\nType=Application\nName=X'),
      isNull,
    );
    expect(
      parseDesktopEntry('x', '[Desktop Entry]\nType=Link\nName=X\nExec=x'),
      isNull,
    );
    expect(stripFieldCodes('app %f --x %% %U'), 'app --x %');
  });

  test('XDG folders: the first one wins on equal ids', () {
    final root = Directory.systemTemp.createTempSync('silica-apps');
    addTearDown(() => root.deleteSync(recursive: true));
    final user = Directory('${root.path}/user/applications')
      ..createSync(recursive: true);
    final system = Directory('${root.path}/system/applications')
      ..createSync(recursive: true);
    File('${user.path}/editor.desktop').writeAsStringSync(
      '[Desktop Entry]\nType=Application\nName=Mio editor\nExec=editor',
    );
    File('${system.path}/editor.desktop').writeAsStringSync(
      '[Desktop Entry]\nType=Application\nName=Editor\nExec=editor',
    );
    File('${system.path}/calc.desktop').writeAsStringSync(
      '[Desktop Entry]\nType=Application\nName=Calcolatrice\nExec=calc',
    );
    final entries = loadDesktopEntries(
      env: {
        'HOME': root.path,
        'XDG_DATA_HOME': '${root.path}/user',
        'XDG_DATA_DIRS': '${root.path}/system',
      },
    );
    expect(entries.keys.toSet(), {'editor', 'calc'});
    expect(entries['editor']!.name, 'Mio editor');
  });

  test('Dock: pinned, then open unpinned, separator, folder and trash', () {
    const files = DesktopEntry(
      id: 'files',
      name: 'File',
      exec: 'files',
      wmClass: 'files',
    );
    const editor = DesktopEntry(id: 'editor', name: 'Editor', exec: 'editor');
    final entries = {'files': files, 'editor': editor};
    final items = dockItems(
      entries,
      ['files'],
      const [
        Toplevel(id: 'a', title: 'Documento', appId: 'editor'),
        Toplevel(id: 'b', title: 'Casa', appId: 'files'),
      ],
    );
    expect(
      [for (final i in items) i.label],
      ['File', 'Editor', '', 'Download', 'Cestino'],
    );
    expect(items[0].running, isTrue);
    expect(items[0].windows, ['b']);
    expect(items[1].windows, ['a']);
    expect(items[2].kind, ItemKind.separator);
  });

  test(
    'recent apps (D-08): second separator, open unpinned and closed, at most 3',
    () {
      DesktopEntry app(String id) => DesktopEntry(id: id, name: id, exec: id);
      final entries = {
        for (final id in ['files', 'editor', 'calc', 'mail', 'foto'])
          id: app(id),
      };
      final items = dockItems(
        entries,
        ['files'],
        const [Toplevel(id: 'a', title: 'Documento', appId: 'editor')],
        recents: ['calc', 'mail', 'foto'],
      );
      expect(
        [for (final i in items) i.label],
        ['files', '', 'editor', 'calc', 'mail', '', 'Download', 'Cestino'],
      );
      expect(items[3].running, isFalse);
      // Without the option: a single separator, the closed ones are not there.
      final off = dockItems(entries, ['files'], const []);
      expect(
        [for (final i in off) i.label],
        ['files', '', 'Download', 'Cestino'],
      );
    },
  );

  test('the Dock options do not erase the pinned apps', () {
    final dir = Directory.systemTemp.createTempSync('dock');
    addTearDown(() => dir.deleteSync(recursive: true));
    final env = {'HOME': dir.path};
    savePinned(['files', 'editor'], env: env);
    saveDockOption('recents', true, env: env);
    expect(readDockConfig(env: env)['pinned'], ['files', 'editor']);
    expect(dockFlag('recents', fallback: false, env: env), isTrue);
    expect(dockFlag('magnify', fallback: true, env: env), isTrue);
  });

  test(
    'Exec with files: %f, %F, %u, %U quoted; without codes it takes none',
    () {
      expect(
        execWithFiles('testo %F', ['/tmp/a.txt', "/tmp/l'altro.txt"]),
        "testo '/tmp/a.txt' '/tmp/l'\\''altro.txt'",
      );
      expect(
        execWithFiles('apri %f --nuovo', ['/a', '/b']),
        "apri '/a' --nuovo",
      );
      expect(execWithFiles('web %U', ['/x y']), "web 'file:///x%20y'");
      expect(execWithFiles('web %u %i', ['/x']), "web 'file:///x'");
      expect(execWithFiles('calcolatrice', ['/a']), isNull);
      expect(execWithFiles('cento %%F', ['/a']), isNull);
      expect(execWithFiles('testo %F', []), isNull);
    },
  );

  test('into the Trash by hand: .trashinfo file, equal names numbered', () async {
    final home = Directory.systemTemp.createTempSync('cestino');
    addTearDown(() => home.deleteSync(recursive: true));
    final a = File('${home.path}/a b.txt')..writeAsStringSync('1');
    await moveToTrash(
      [a.path],
      env: {'HOME': home.path},
      now: DateTime(2026, 10, 14, 9, 30),
    );
    final trash = '${home.path}/.local/share/Trash';
    expect(a.existsSync(), isFalse);
    expect(File('$trash/files/a b.txt').readAsStringSync(), '1');
    expect(
      File('$trash/info/a b.txt.trashinfo').readAsStringSync(),
      '[Trash Info]\nPath=${Uri.file(a.path).path}\nDeletionDate=2026-10-14T09:30:00\n',
    );
    File(a.path).writeAsStringSync('2');
    await moveToTrash([a.path], env: {'HOME': home.path});
    expect(File('$trash/files/a b.txt 2').readAsStringSync(), '2');
  });
}
