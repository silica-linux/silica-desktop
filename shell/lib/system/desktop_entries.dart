// The installed apps, from .desktop files (freedesktop Desktop Entry specification): the
// applications/ folders of XDG_DATA_HOME and XDG_DATA_DIRS, the first one wins on equal ids.

import 'dart:io';

import 'package:silica_ui/silica_ui.dart' show xdgDataDirs;

export 'package:silica_ui/silica_ui.dart' show xdgDataDirs;

/// A launchable app.
class DesktopEntry {
  const DesktopEntry({
    required this.id,
    required this.name,
    required this.exec,
    this.rawExec,
    this.icon,
    this.wmClass,
    this.keywords = const [],
    this.silicaIcon,
    this.categories = const [],
    this.actions = const [],
    this.mimeTypes = const [],
  });

  /// File name without `.desktop` (e.g. `org.gnome.Nautilus`).
  final String id;
  final String name;

  /// Command without field codes (`%f`, `%U`, …), ready for `sh -c`.
  final String exec;

  /// Exec as written in the entry, with field codes: to open files ([execWithFiles]).
  final String? rawExec;
  final String? icon;
  final String? wmClass;
  final List<String> keywords;

  /// The layered silica-icons icon chosen by the app (`X-Silica-Icon`).
  final String? silicaIcon;

  /// The freedesktop categories (`Categories`, e.g. `Utility`, `Office`).
  final List<String> categories;

  /// The types of file it opens (`MimeType`).
  final List<String> mimeTypes;

  /// The entry's own actions (`Actions` and their `[Desktop Action …]` groups).
  final List<DesktopAction> actions;

  /// The window with this app id belongs to this app.
  bool matchesAppId(String appId) {
    if (appId.isEmpty) return false;
    final lower = appId.toLowerCase();
    return id.toLowerCase() == lower ||
        (wmClass?.toLowerCase() == lower) ||
        id.toLowerCase().split('.').last == lower ||
        exec.split(' ').first.split('/').last.toLowerCase() == lower;
  }
}

/// An action of an app (a `[Desktop Action <id>]` group): a name and its command.
class DesktopAction {
  const DesktopAction({
    required this.id,
    required this.name,
    required this.exec,
    this.icon,
  });

  final String id;
  final String name;

  /// Command without field codes, ready for `sh -c`.
  final String exec;
  final String? icon;
}

/// The command that opens `paths` with the app (Desktop Entry specification, "Exec"): `%f`/`%u`
/// with the first, `%F`/`%U` with all of them (`file://` URIs for `%u`/`%U`), quoted for `sh -c`.
/// Null if Exec takes no files: the app does not accept them.
String? execWithFiles(String exec, List<String> paths) {
  if (paths.isEmpty) return null;
  String quote(String s) => "'${s.replaceAll("'", r"'\''")}'";
  String uri(String p) => Uri.file(p).toString();
  final out = StringBuffer();
  var takes = false;
  for (var i = 0; i < exec.length; i++) {
    if (exec[i] != '%' || i + 1 >= exec.length) {
      out.write(exec[i]);
      continue;
    }
    i++;
    switch (exec[i]) {
      case '%':
        out.write('%');
      case 'f':
        takes = true;
        out.write(quote(paths.first));
      case 'F':
        takes = true;
        out.write(paths.map(quote).join(' '));
      case 'u':
        takes = true;
        out.write(quote(uri(paths.first)));
      case 'U':
        takes = true;
        out.write(paths.map((p) => quote(uri(p))).join(' '));
      default:
      // The other codes are dropped.
    }
  }
  if (!takes) return null;
  return out.toString().replaceAll(RegExp(r'\s+'), ' ').trim();
}

/// Removes the Exec field codes (`%f %F %u %U %d %D %n %N %i %c %k %v %m`) and `%%`.
String stripFieldCodes(String exec) {
  final out = StringBuffer();
  for (var i = 0; i < exec.length; i++) {
    if (exec[i] == '%' && i + 1 < exec.length) {
      if (exec[i + 1] == '%') out.write('%');
      i++;
      continue;
    }
    out.write(exec[i]);
  }
  return out.toString().replaceAll(RegExp(r'\s+'), ' ').trim();
}

/// The file manager: a system component, always first in the Dock, neither dragged nor
/// removed; Faro and the Launcher list it like any app.
// TODO(#142): start it with the session and restart it when it exits, once the embedder keeps
// an app alive without windows (today it would open a window at login).
const String fileManagerId = 'dev.frascella.Mustache';

/// Reads an entry; null if it is not an app to show (NoDisplay, Hidden, no Exec).
DesktopEntry? parseDesktopEntry(
  String id,
  String text, {
  String locale = 'en',
}) {
  final values = <String, String>{};
  final groups = <String, Map<String, String>>{};
  const actionHeader = '[Desktop Action ';
  Map<String, String>? group;
  for (final raw in text.split('\n')) {
    final line = raw.trim();
    if (line.startsWith('[')) {
      group = line == '[Desktop Entry]'
          ? values
          : line.startsWith(actionHeader) && line.endsWith(']')
          ? groups.putIfAbsent(
              line.substring(actionHeader.length, line.length - 1),
              () => {},
            )
          : null;
      continue;
    }
    if (group == null || line.startsWith('#')) continue;
    final eq = line.indexOf('=');
    if (eq <= 0) continue;
    group.putIfAbsent(
      line.substring(0, eq).trim(),
      () => line.substring(eq + 1).trim(),
    );
  }
  if (values['Type'] != 'Application' ||
      values['NoDisplay'] == 'true' ||
      values['Hidden'] == 'true') {
    return null;
  }
  final exec = values['Exec'];
  final name = values['Name[$locale]'] ?? values['Name'];
  if (exec == null || name == null) return null;
  return DesktopEntry(
    id: id,
    name: name,
    exec: stripFieldCodes(exec),
    rawExec: exec,
    icon: values['Icon'],
    wmClass: values['StartupWMClass'],
    keywords: (values['Keywords[$locale]'] ?? values['Keywords'] ?? '')
        .split(';')
        .where((k) => k.isNotEmpty)
        .toList(),
    silicaIcon: values['X-Silica-Icon'],
    categories: (values['Categories'] ?? '')
        .split(';')
        .where((c) => c.isNotEmpty)
        .toList(),
    mimeTypes: (values['MimeType'] ?? '')
        .split(';')
        .where((m) => m.isNotEmpty)
        .toList(),
    actions: [
      for (final action in (values['Actions'] ?? '').split(';'))
        if (groups[action] case final g?)
          if ((g['Name[$locale]'] ?? g['Name']) case final actionName?)
            if (g['Exec'] case final actionExec?)
              DesktopAction(
                id: action,
                name: actionName,
                exec: stripFieldCodes(actionExec),
                icon: g['Icon'],
              ),
    ],
  );
}

/// All installed apps, by id.
/// The installed apps, with names in `locale` (`Name[it]`), English by default.
Map<String, DesktopEntry> loadDesktopEntries({
  Map<String, String>? env,
  String locale = 'en',
}) {
  final entries = <String, DesktopEntry>{};
  final seen = <String>{};
  for (final dir in xdgDataDirs(env ?? Platform.environment)) {
    final apps = Directory('$dir/applications');
    if (!apps.existsSync()) continue;
    for (final file in apps.listSync(recursive: true).whereType<File>()) {
      if (!file.path.endsWith('.desktop')) continue;
      final relative = file.path.substring(apps.path.length + 1);
      final id = relative
          .substring(0, relative.length - '.desktop'.length)
          .replaceAll('/', '-');
      // The first folder wins, even if the app is hidden there.
      if (!seen.add(id)) continue;
      try {
        final entry = parseDesktopEntry(
          id,
          file.readAsStringSync(),
          locale: locale,
        );
        if (entry != null) entries[id] = entry;
      } on FileSystemException {
        continue;
      }
    }
  }
  return entries;
}
