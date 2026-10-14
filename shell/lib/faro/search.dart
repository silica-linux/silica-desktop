// Faro's search over the installed apps.

import '../system/desktop_entries.dart';

/// The apps matching `query`, most relevant first: name starting with the query, then a
/// word of the name, then a name containing it, then the keywords.
List<DesktopEntry> searchApps(Iterable<DesktopEntry> entries, String query) {
  final q = query.trim().toLowerCase();
  if (q.isEmpty) return const [];
  int? rank(DesktopEntry e) {
    final name = e.name.toLowerCase();
    if (name.startsWith(q)) return 0;
    if (name.split(RegExp(r'[\s\-_.]+')).any((w) => w.startsWith(q))) return 1;
    if (name.contains(q)) return 2;
    if (e.keywords.any((k) => k.toLowerCase().startsWith(q))) return 3;
    return null;
  }

  final scored =
      [
        for (final e in entries)
          if (rank(e) case final r?) (r, e),
      ]..sort((a, b) {
        final byRank = a.$1.compareTo(b.$1);
        return byRank != 0
            ? byRank
            : a.$2.name.toLowerCase().compareTo(b.$2.name.toLowerCase());
      });
  // Two entries with the same name and command (different packages of the same app)
  // are shown once.
  final seen = <String>{};
  return [
    for (final (_, e) in scored)
      if (seen.add('${e.name}\u0000${e.exec}')) e,
  ];
}
