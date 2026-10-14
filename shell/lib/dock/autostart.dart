// Login items (freedesktop Desktop Application Autostart): the Dock's "Open at Login" writes
// them, and the Dock starts them once per session (Silica has no other autostart runner).

import 'dart:io';

import '../system/desktop_entries.dart';
import '../system/shell_channel.dart';

String _configHome(Map<String, String> e) =>
    e['XDG_CONFIG_HOME'] ?? '${e['HOME'] ?? ''}/.config';

/// The folders with login items, most important first: the user's, then the system's.
List<String> autostartDirs(Map<String, String> env) => [
  '${_configHome(env)}/autostart',
  for (final dir in (env['XDG_CONFIG_DIRS'] ?? '/etc/xdg').split(':'))
    if (dir.isNotEmpty) '$dir/autostart',
];

/// A login item worth starting in Silica: its command, or null.
String? autostartCommand(String text, {bool Function(String)? onPath}) {
  final values = <String, String>{};
  var inEntry = false;
  for (final raw in text.split('\n')) {
    final line = raw.trim();
    if (line.startsWith('[')) {
      inEntry = line == '[Desktop Entry]';
      continue;
    }
    if (!inEntry || line.startsWith('#')) continue;
    final eq = line.indexOf('=');
    if (eq <= 0) continue;
    values.putIfAbsent(
      line.substring(0, eq).trim(),
      () => line.substring(eq + 1).trim(),
    );
  }
  List<String> list(String key) =>
      (values[key] ?? '').split(';').where((v) => v.isNotEmpty).toList();
  if (values['Type'] != 'Application' ||
      values['Hidden'] == 'true' ||
      values['X-GNOME-Autostart-enabled'] == 'false') {
    return null;
  }
  final only = list('OnlyShowIn');
  if (only.isNotEmpty && !only.contains('Silica')) return null;
  if (list('NotShowIn').contains('Silica')) return null;
  if (values['TryExec'] case final tryExec? when tryExec.isNotEmpty) {
    if (!(onPath ?? _onPath)(tryExec)) return null;
  }
  final exec = values['Exec'];
  return exec == null || exec.isEmpty ? null : stripFieldCodes(exec);
}

bool _onPath(String program) {
  if (program.startsWith('/')) return File(program).existsSync();
  final path = Platform.environment['PATH'] ?? '/usr/bin:/bin';
  return path
      .split(':')
      .any((dir) => dir.isNotEmpty && File('$dir/$program').existsSync());
}

/// The commands of all login items: a user file hides the system one with the same name.
List<String> autostartCommands({Map<String, String>? env}) {
  final e = env ?? Platform.environment;
  final seen = <String>{};
  final commands = <String>[];
  for (final dir in autostartDirs(e)) {
    final folder = Directory(dir);
    if (!folder.existsSync()) continue;
    final files =
        folder
            .listSync()
            .whereType<File>()
            .where((f) => f.path.endsWith('.desktop'))
            .toList()
          ..sort((a, b) => a.path.compareTo(b.path));
    for (final file in files) {
      final name = file.uri.pathSegments.last;
      if (!seen.add(name)) continue;
      try {
        if (autostartCommand(file.readAsStringSync()) case final command?) {
          commands.add(command);
        }
      } on FileSystemException {
        continue;
      }
    }
  }
  return commands;
}

/// Starts the login items once per session: a marker in the runtime folder keeps a restarted
/// Dock from starting them again.
void runAutostartOnce({Map<String, String>? env}) {
  final e = env ?? Platform.environment;
  final runtime = e['XDG_RUNTIME_DIR'];
  if (runtime == null || runtime.isEmpty) return;
  final marker = File(
    '$runtime/silica-autostart-${e['WAYLAND_DISPLAY'] ?? 'session'}',
  );
  try {
    if (marker.existsSync()) return;
    marker.writeAsStringSync('');
  } on FileSystemException {
    return;
  }
  for (final command in autostartCommands(env: e)) {
    ShellChannel.instance.launch(command);
  }
}

/// The user's login item for app `id`.
File _userItem(String id, Map<String, String> env) =>
    File('${_configHome(env)}/autostart/$id.desktop');

/// The app starts at login.
bool opensAtLogin(String id, {Map<String, String>? env}) {
  final e = env ?? Platform.environment;
  for (final dir in autostartDirs(e)) {
    final file = File('$dir/$id.desktop');
    if (!file.existsSync()) continue;
    try {
      return autostartCommand(file.readAsStringSync(), onPath: (_) => true) !=
          null;
    } on FileSystemException {
      return false;
    }
  }
  return false;
}

/// "Open at Login" on or off: a user login item with the app's command, or one that hides the
/// system item with the same name.
void setOpensAtLogin(
  String id,
  String exec,
  String name, {
  required bool on,
  Map<String, String>? env,
}) {
  final e = env ?? Platform.environment;
  final file = _userItem(id, e);
  try {
    file.parent.createSync(recursive: true);
    file.writeAsStringSync(
      '[Desktop Entry]\n'
      'Type=Application\n'
      'Name=$name\n'
      'Exec=$exec\n'
      '${on ? '' : 'Hidden=true\n'}',
    );
  } on FileSystemException {
    // Not writable: the choice is lost.
  }
}
