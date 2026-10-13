// La ricerca di Faro sulle app installate.

import '../system/desktop_entries.dart';

/// Le app che corrispondono a `query`, dalle più pertinenti: nome che inizia con la
/// ricerca, poi una parola del nome, poi il nome che la contiene, poi le parole chiave.
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
  // Due voci con lo stesso nome e lo stesso comando (pacchetti diversi della stessa app)
  // si mostrano una volta.
  final seen = <String>{};
  return [
    for (final (_, e) in scored)
      if (seen.add('${e.name}\u0000${e.exec}')) e,
  ];
}
