// Le app installate, dai file .desktop (specifica freedesktop Desktop Entry): cartelle
// applications/ di XDG_DATA_HOME e XDG_DATA_DIRS, la prima vince a parità di id.

import 'dart:io';

/// Un'app avviabile.
class DesktopEntry {
  const DesktopEntry({
    required this.id,
    required this.name,
    required this.exec,
    this.icon,
    this.wmClass,
    this.keywords = const [],
    this.silicaIcon,
    this.categories = const [],
  });

  /// Nome del file senza `.desktop` (es. `org.gnome.Nautilus`).
  final String id;
  final String name;

  /// Comando senza i codici di campo (`%f`, `%U`, …), pronto per `sh -c`.
  final String exec;
  final String? icon;
  final String? wmClass;
  final List<String> keywords;

  /// L'icona a livelli di silica-icons scelta dall'app (`X-Silica-Icon`).
  final String? silicaIcon;

  /// Le categorie freedesktop (`Categories`, es. `Utility`, `Office`).
  final List<String> categories;

  /// La finestra con questo app id appartiene a questa app.
  bool matchesAppId(String appId) {
    if (appId.isEmpty) return false;
    final lower = appId.toLowerCase();
    return id.toLowerCase() == lower ||
        (wmClass?.toLowerCase() == lower) ||
        id.toLowerCase().split('.').last == lower ||
        exec.split(' ').first.split('/').last.toLowerCase() == lower;
  }
}

/// Toglie i codici di campo di Exec (`%f %F %u %U %d %D %n %N %i %c %k %v %m`) e `%%`.
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

/// Legge una voce; null se non è un'app da mostrare (NoDisplay, Hidden, niente Exec).
DesktopEntry? parseDesktopEntry(
  String id,
  String text, {
  String locale = 'it',
}) {
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
  );
}

/// Le cartelle dei dati XDG, dalla più importante.
List<String> xdgDataDirs(Map<String, String> env) {
  final home = env['HOME'] ?? '';
  final dataHome = env['XDG_DATA_HOME'] ?? '$home/.local/share';
  final dataDirs = (env['XDG_DATA_DIRS'] ?? '/usr/local/share:/usr/share')
      .split(':')
      .where((d) => d.isNotEmpty);
  return [dataHome, ...dataDirs];
}

/// Tutte le app installate, per id.
Map<String, DesktopEntry> loadDesktopEntries({Map<String, String>? env}) {
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
      // La prima cartella vince, anche se lì l'app è nascosta.
      if (!seen.add(id)) continue;
      try {
        final entry = parseDesktopEntry(id, file.readAsStringSync());
        if (entry != null) entries[id] = entry;
      } on FileSystemException {
        continue;
      }
    }
  }
  return entries;
}
