// Which silica-icons icon an app gets: only the one declared by its .desktop entry.
// The rest (manifests, special folders, drawing) is in silica_ui (`SilicaIcon`).

import 'package:silica_ui/silica_ui.dart';

import 'desktop_entries.dart';

/// The Silica icon of an app: only the one the entry declares with `X-Silica-Icon` (Silica's
/// own apps); the others keep the icon of their theme.
LayeredIcon? silicaIconFor(DesktopEntry entry, {Map<String, String>? env}) {
  final id = entry.silicaIcon;
  return id == null ? null : findSilicaIcon(id, env: env);
}
