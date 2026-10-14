import 'package:flutter_test/flutter_test.dart';
import 'package:silica_shell/dock/apps.dart';
import 'package:silica_shell/system/app_registry.dart';
import 'package:silica_shell/system/desktop_entries.dart';

void main() {
  test('the registry lists live sockets and parked apps', () {
    final apps = registeredApps([
      'dev.frascella.Mustache.4242.sock',
      'dev.frascella.Mail.17.sock',
      'dev.frascella.Clock.parked',
      'dev.frascella.Gone.99.sock',
      'notes.txt',
      '../evil.1.sock',
    ], alive: (pid) => pid != 99);
    expect(apps, {
      'dev.frascella.Mustache',
      'dev.frascella.Mail',
      'dev.frascella.Clock',
    });
  });

  test('a running app without windows keeps its dot in the Dock', () {
    const files = DesktopEntry(id: 'files', name: 'Files', exec: 'files');
    const mail = DesktopEntry(id: 'mail', name: 'Mail', exec: 'mail');
    final items = dockItems(
      {'files': files, 'mail': mail},
      ['files'],
      const [],
      registered: {'files', 'mail'},
    );
    final files0 = items.firstWhere((i) => i.label == 'Files');
    final mail0 = items.firstWhere((i) => i.label == 'Mail');
    // Pinned and running without windows; not pinned, still in the Dock while it runs.
    expect(files0.running, isTrue);
    expect(files0.windows, isEmpty);
    expect(mail0.running, isTrue);
    expect(
      dockItems({'files': files}, ['files'], const []).first.running,
      isFalse,
    );
  });
}
