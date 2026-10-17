import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:silica_shell/dock/autostart.dart';
import 'package:silica_shell/dock/recent_documents.dart';
import 'package:silica_shell/system/desktop_entries.dart';

void main() {
  test('login items: what Silica starts, and Open at Login on and off', () {
    expect(
      autostartCommand(
        '[Desktop Entry]\nType=Application\nExec=foo --bar %U\n',
      ),
      'foo --bar',
    );
    expect(
      autostartCommand(
        '[Desktop Entry]\nType=Application\nExec=foo\nOnlyShowIn=GNOME;\n',
      ),
      isNull,
    );
    expect(
      autostartCommand(
        '[Desktop Entry]\nType=Application\nExec=foo\nNotShowIn=Silica;\n',
      ),
      isNull,
    );
    expect(
      autostartCommand(
        '[Desktop Entry]\nType=Application\nExec=foo\nHidden=true\n',
      ),
      isNull,
    );
    expect(
      autostartCommand(
        '[Desktop Entry]\nType=Application\nExec=foo\nTryExec=nope\n',
        onPath: (_) => false,
      ),
      isNull,
    );

    final config = Directory.systemTemp.createTempSync('autostart');
    addTearDown(() => config.deleteSync(recursive: true));
    final env = {
      'XDG_CONFIG_HOME': config.path,
      'XDG_CONFIG_DIRS': '${config.path}/system',
    };
    Directory('${config.path}/system/autostart').createSync(recursive: true);
    File('${config.path}/system/autostart/agent.desktop')
        .writeAsStringSync('[Desktop Entry]\nType=Application\nExec=agent\n');
    expect(autostartCommands(env: env), ['agent']);
    expect(opensAtLogin('agent', env: env), isTrue);
    // Off: a user item hides the system one.
    setOpensAtLogin('agent', 'agent', 'Agent', on: false, env: env);
    expect(opensAtLogin('agent', env: env), isFalse);
    expect(autostartCommands(env: env), isEmpty);
    setOpensAtLogin('dev.x.Notes', 'notes', 'Notes', on: true, env: env);
    expect(opensAtLogin('dev.x.Notes', env: env), isTrue);
    expect(autostartCommands(env: env), ['notes']);
  });

  test('login items start only in a real Silica session', () {
    final runtime = Directory.systemTemp.createTempSync('runtime');
    addTearDown(() => runtime.deleteSync(recursive: true));
    final env = {
      'XDG_RUNTIME_DIR': runtime.path,
      'WAYLAND_DISPLAY': 'wayland-9',
      'XDG_CURRENT_DESKTOP': 'GNOME',
      'XDG_CONFIG_HOME': runtime.path,
      'XDG_CONFIG_DIRS': runtime.path,
    };
    runAutostartOnce(env: env);
    expect(
      File('${runtime.path}/silica-autostart-wayland-9').existsSync(),
      isFalse,
    );
    runAutostartOnce(env: {...env, 'XDG_CURRENT_DESKTOP': 'Silica'});
    expect(
      File('${runtime.path}/silica-autostart-wayland-9').existsSync(),
      isTrue,
    );
  });

  test('recent documents of an app, newest first, only existing files', () {
    const entry = DesktopEntry(
      id: 'org.example.Editor',
      name: 'Editor',
      exec: 'editor',
    );
    const xbel = '''
<xbel version="1.0" xmlns:bookmark="http://www.freedesktop.org/standards/desktop-bookmarks">
  <bookmark href="file:///home/u/a%20b.txt" modified="2026-10-01T10:00:00Z">
    <info><metadata owner="http://freedesktop.org">
      <bookmark:applications>
        <bookmark:application name="Editor" exec="&apos;editor %u&apos;" modified="2026-10-02T10:00:00Z" count="1"/>
      </bookmark:applications>
    </metadata></info>
  </bookmark>
  <bookmark href="file:///home/u/new.txt" modified="2026-10-05T10:00:00Z">
    <info><metadata owner="http://freedesktop.org">
      <bookmark:applications>
        <bookmark:application name="Other" exec="&apos;editor %u&apos;" modified="2026-10-05T10:00:00Z" count="1"/>
      </bookmark:applications>
    </metadata></info>
  </bookmark>
  <bookmark href="file:///home/u/gone.txt" modified="2026-10-09T10:00:00Z">
    <info><metadata owner="http://freedesktop.org">
      <bookmark:applications>
        <bookmark:application name="Editor" exec="editor" modified="2026-10-09T10:00:00Z" count="1"/>
      </bookmark:applications>
    </metadata></info>
  </bookmark>
  <bookmark href="file:///home/u/viewer.png" modified="2026-10-09T10:00:00Z">
    <info><metadata owner="http://freedesktop.org">
      <bookmark:applications>
        <bookmark:application name="Viewer" exec="viewer" modified="2026-10-09T10:00:00Z" count="1"/>
      </bookmark:applications>
    </metadata></info>
  </bookmark>
</xbel>''';
    final docs = recentDocumentsIn(
      xbel,
      entry,
      exists: (p) => !p.endsWith('gone.txt'),
    );
    expect(
      [for (final d in docs) d.path],
      ['/home/u/new.txt', '/home/u/a b.txt'],
    );
  });
}
