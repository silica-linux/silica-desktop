// Quale icona di silica-icons spetta a un'app: solo quella dichiarata dalla voce .desktop.
// Il resto (manifest, cartelle speciali, disegno) è in silica_ui (`SilicaIcon`).

import 'package:silica_ui/silica_ui.dart';

import 'desktop_entries.dart';

/// L'icona di Silica di un'app: solo quella che la voce dichiara con `X-Silica-Icon` (le
/// app di Silica); le altre restano con l'icona del loro tema.
LayeredIcon? silicaIconFor(DesktopEntry entry, {Map<String, String>? env}) {
  final id = entry.silicaIcon;
  return id == null ? null : findSilicaIcon(id, env: env);
}
