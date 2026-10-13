import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:silica_shell/dock/apps.dart';
import 'package:silica_shell/dock/model.dart';
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
    'una voce .desktop: nome localizzato, Exec senza codici, azioni ignorate',
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

  test('voci nascoste o senza Exec non sono app', () {
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

  test('le cartelle XDG: la prima vince a parità di id', () {
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

  test(
    'Dock: fissate, poi le aperte non fissate, separatore, cartella e cestino',
    () {
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
    },
  );
}
