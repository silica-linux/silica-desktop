// An app's recent documents for its Dock menu (D-04): the freedesktop recently-used list
// (~/.local/share/recently-used.xbel), the files that app opened, most recent first.

import 'dart:io';

import '../system/desktop_entries.dart';

/// A recent document: the file and when it was last used.
typedef RecentDocument = ({String path, DateTime used});

String _recentFile(Map<String, String> env) {
  final data = env['XDG_DATA_HOME'];
  final base = (data == null || data.isEmpty)
      ? '${env['HOME'] ?? ''}/.local/share'
      : data;
  return '$base/recently-used.xbel';
}

String _unescape(String s) => s
    .replaceAll('&quot;', '"')
    .replaceAll('&apos;', "'")
    .replaceAll('&lt;', '<')
    .replaceAll('&gt;', '>')
    .replaceAll('&amp;', '&');

/// The documents in `xbel` opened by the app of `entry`, most recent first, at most `limit`;
/// only local files that still exist.
List<RecentDocument> recentDocumentsIn(
  String xbel,
  DesktopEntry entry, {
  int limit = 10,
  bool Function(String path)? exists,
}) {
  var program = entry.exec.trim().split(RegExp(r'\s+')).first.split('/').last;
  // Silica's apps all start through the embedder: its name says nothing about the app.
  if (program == 'silica-app' || program == 'silica-embedder') program = '';
  final names = {
    entry.name.toLowerCase(),
    entry.id.toLowerCase(),
    if (program.isNotEmpty) program,
  };
  final found = <RecentDocument>[];
  final bookmark = RegExp(r'<bookmark\b([^>]*)>(.*?)</bookmark>', dotAll: true);
  final attr = RegExp(r'(\w+)="([^"]*)"');
  final application = RegExp(r'<bookmark:application\b([^>]*)/?>');
  for (final b in bookmark.allMatches(xbel)) {
    final attrs = {
      for (final a in attr.allMatches(b.group(1) ?? '')) a.group(1): a.group(2),
    };
    final href = attrs['href'];
    if (href == null || !href.startsWith('file://')) continue;
    DateTime? used;
    var byApp = false;
    for (final app in application.allMatches(b.group(2) ?? '')) {
      final a = {
        for (final m in attr.allMatches(app.group(1) ?? ''))
          m.group(1): _unescape(m.group(2) ?? ''),
      };
      final name = (a['name'] ?? '').toLowerCase();
      final exec = (a['exec'] ?? '').replaceAll("'", '').trim();
      final execProgram = exec.split(RegExp(r'\s+')).first.split('/').last;
      if (names.contains(name) ||
          (execProgram.isNotEmpty && execProgram == program)) {
        byApp = true;
        used = DateTime.tryParse(a['modified'] ?? '') ?? used;
      }
    }
    if (!byApp) continue;
    used ??= DateTime.tryParse(attrs['modified'] ?? attrs['visited'] ?? '');
    final String path;
    try {
      path = Uri.parse(_unescape(href)).toFilePath();
    } on FormatException {
      continue;
    } on UnsupportedError {
      continue;
    }
    if (!(exists ?? (p) => File(p).existsSync())(path)) continue;
    found.add((
      path: path,
      used: used ?? DateTime.fromMillisecondsSinceEpoch(0),
    ));
  }
  found.sort((a, b) => b.used.compareTo(a.used));
  final unique = <String>{};
  return [
    for (final d in found)
      if (unique.add(d.path)) d,
  ].take(limit).toList();
}

/// The app's recent documents from the user's list.
List<RecentDocument> recentDocuments(
  DesktopEntry entry, {
  Map<String, String>? env,
}) {
  final file = File(_recentFile(env ?? Platform.environment));
  try {
    if (!file.existsSync()) return const [];
    return recentDocumentsIn(file.readAsStringSync(), entry);
  } on FileSystemException {
    return const [];
  }
}
