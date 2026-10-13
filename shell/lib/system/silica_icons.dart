// Le icone a livelli del repo silica-icons (specifica: docs/icon-format.md di quel repo):
// dove sono installate, il manifest icon.json e quale icona spetta a un'app. Le icone sono
// file dell'utente o del sistema (licenza CC BY-SA 4.0), non asset di questo repository.

import 'dart:convert';
import 'dart:io';
import 'dart:ui' show Color, Rect;

import 'desktop_entries.dart';

/// `app`: la shell ritaglia il corpo e aggiunge bordo e ombra; `file`: forma libera.
enum IconKind { app, file }

/// Un livello SVG sulla tela 1024, con la variante per i temi scuri se c'è.
class IconLayer {
  const IconLayer(this.path, {this.darkPath, this.shadow = false});

  final String path;
  final String? darkPath;

  /// La shell disegna sotto il livello un'ombra neutra (`"shadow": "neutral"`).
  final bool shadow;

  String pathFor({required bool dark}) => dark ? darkPath ?? path : path;
}

/// Un testo disegnato dalla shell (mese e giorno del calendario).
class IconText {
  const IconText({
    required this.size,
    required this.weight,
    required this.tracking,
    required this.color,
    required this.darkColor,
    required this.box,
    this.maxWidth,
    this.upper = false,
  });

  final double size;
  final int weight;
  final double tracking;
  final Color color;
  final Color darkColor;

  /// Riquadro sulla tela 1024; il testo è centrato.
  final Rect box;
  final double? maxWidth;
  final bool upper;
}

/// Contenuto aggiornato a runtime.
sealed class IconDynamic {
  const IconDynamic();
}

/// Lancette che ruotano attorno a (512, 512), con il perno sopra.
class ClockHands extends IconDynamic {
  const ClockHands({
    required this.hour,
    required this.minute,
    required this.second,
    required this.cap,
  });

  final IconLayer hour;
  final IconLayer minute;
  final IconLayer? second;
  final IconLayer cap;
}

/// Mese e giorno scritti sul primo piano.
class CalendarText extends IconDynamic {
  const CalendarText({required this.month, required this.day});

  final IconText month;
  final IconText day;
}

/// Una fascia di dimensioni della cartella generica: da `minSize` pt (lato del corpo) in su il
/// simbolo usa `weight`, scalato di `scale` attorno al centro del riquadro.
class SymbolBand {
  const SymbolBand({
    required this.minSize,
    required this.weight,
    this.scale = 1,
    this.highlight = true,
  });

  final double minSize;

  /// `light`, `regular` o `semibold`: la cartella di `symbols/`.
  final String weight;
  final double scale;

  /// Disegna il riflesso dello scavo (sotto 64 pt no: mezzo pixel impasterebbe il simbolo).
  final bool highlight;
}

/// L'effetto inciso del simbolo in un tema: il riflesso spostato in basso di `offset`, poi il
/// simbolo.
class SymbolEffect {
  const SymbolEffect({
    required this.color,
    required this.opacity,
    required this.highlight,
    required this.highlightOpacity,
    required this.offset,
  });

  final Color color;
  final double opacity;
  final Color highlight;
  final double highlightOpacity;

  /// Sulla tela 1024.
  final double offset;
}

/// La tinta della cartella: la luminanza del disegno composto va da tinta × `shade` (a `low`)
/// alla tinta schiarita verso il bianco di `glow` (a `high`).
class TintRamp {
  const TintRamp({
    required this.low,
    required this.high,
    required this.shade,
    required this.glow,
  });

  final double low;
  final double high;
  final double shade;
  final double glow;

  /// La matrice 4 × 5 di `ColorFilter.matrix` (scostamenti in 0–255) per la tinta.
  List<double> matrix(Color tint) {
    const lum = [0.2126, 0.7152, 0.0722];
    final rows = <double>[];
    for (final t in [tint.r, tint.g, tint.b]) {
      final from = t * shade;
      final to = t + (1 - t) * glow;
      final k = (to - from) / (high - low);
      rows.addAll([
        k * lum[0],
        k * lum[1],
        k * lum[2],
        0,
        (from - k * low) * 255,
      ]);
    }
    return [...rows, 0, 0, 0, 1, 0];
  }
}

/// La cartella generica: un simbolo di `symbols/` inciso sul pannello e una tinta.
class FolderSymbol extends IconDynamic {
  const FolderSymbol({
    required this.box,
    required this.bands,
    required this.light,
    required this.dark,
    required this.symbolsDir,
    this.lightTint,
    this.darkTint,
    this.labels = const {},
    this.folders = const {},
    this.fallback,
  });

  /// La tela 24 × 24 del simbolo sulla tela 1024.
  final Rect box;

  /// Dalla più grande alla più piccola.
  final List<SymbolBand> bands;
  final SymbolEffect light;
  final SymbolEffect dark;

  /// `symbols/` del repo da cui viene l'icona.
  final String symbolsDir;
  final TintRamp? lightTint;
  final TintRamp? darkTint;

  /// Le etichette con il loro colore (`red` → #f2382e…).
  final Map<String, Color> labels;

  /// Chiave della cartella speciale (DOCUMENTS, HOME…) → nome del simbolo.
  final Map<String, String> folders;

  /// L'icona da usare senza simbolo né tinta (la cartella semplice).
  final String? fallback;

  /// La fascia per un'icona di `side` pt; null sotto l'ultima (niente simbolo).
  SymbolBand? bandFor(double side) {
    for (final band in bands) {
      if (side >= band.minSize) return band;
    }
    return null;
  }

  /// Il file del simbolo `name` nel peso `weight`, se esiste.
  String? symbolFile(String name, String weight) {
    if (!_idPattern.hasMatch(name) || !_weights.contains(weight)) return null;
    final path = '$symbolsDir/$weight/$name.svg';
    return File(path).existsSync() ? path : null;
  }

  /// Un colore `#rrggbb` o il nome di un'etichetta; null se non è né l'uno né l'altro.
  Color? tintFor(String? value) =>
      value == null ? null : labels[value] ?? _parseColor(value);
}

/// Un'icona di silica-icons, pronta da comporre.
class LayeredIcon {
  const LayeredIcon({
    required this.id,
    required this.kind,
    required this.layers,
    this.dynamic,
  });

  final String id;
  final IconKind kind;

  /// Dal basso verso l'alto: per le app il fondo e i primi piani, per i file l'artwork.
  final List<IconLayer> layers;
  final IconDynamic? dynamic;
}

final RegExp _idPattern = RegExp(r'^[a-z0-9][a-z0-9-]{0,63}$');
final RegExp _folderKeyPattern = RegExp(r'^[A-Z][A-Z0-9_]{0,31}$');
final RegExp _labelPattern = RegExp(r'^[a-z][a-z0-9-]{0,31}$');
const Set<String> _weights = {'light', 'regular', 'semibold'};
final RegExp _filePattern = RegExp(r'^[A-Za-z0-9][A-Za-z0-9._-]{0,63}\.svg$');

/// Legge `icon.json` della cartella `dir`; null se il manifest non è valido o un file manca.
LayeredIcon? parseManifest(String dir, String text) {
  Object? json;
  try {
    json = jsonDecode(text);
  } on FormatException {
    return null;
  }
  if (json is! Map || json['format'] != 1) return null;
  final id = json['id'];
  if (id is! String || !_idPattern.hasMatch(id)) return null;
  final kind = switch (json['kind']) {
    'app' => IconKind.app,
    'file' => IconKind.file,
    _ => null,
  };
  if (kind == null) return null;
  final variants = json['variants'];
  final darkMap = variants is Map ? variants['dark'] : null;

  String? file(Object? name) =>
      name is String &&
          _filePattern.hasMatch(name) &&
          File('$dir/$name').existsSync()
      ? '$dir/$name'
      : null;

  final layers = <IconLayer>[];
  final rawLayers = json['layers'];
  if (rawLayers is! List || rawLayers.isEmpty || rawLayers.length > 5)
    return null;
  for (final raw in rawLayers) {
    if (raw is! Map) return null;
    final role = raw['role'];
    final path = file(raw['file']);
    if (role is! String || path == null) return null;
    layers.add(
      IconLayer(
        path,
        darkPath: darkMap is Map ? file(darkMap[role]) : null,
        shadow: raw['shadow'] == 'neutral' || raw['shadow'] == 'chromatic',
      ),
    );
  }
  return LayeredIcon(
    id: id,
    kind: kind,
    layers: layers,
    dynamic: _parseDynamic(json['dynamic'], file, dir),
  );
}

IconDynamic? _parseDynamic(
  Object? raw,
  String? Function(Object?) file,
  String dir,
) {
  if (raw is! Map) return null;
  IconLayer? layer(Object? name) {
    if (name is! String) return null;
    final path = file(name);
    if (path == null) return null;
    final dot = name.lastIndexOf('.');
    return IconLayer(
      path,
      darkPath: file('${name.substring(0, dot)}-dark${name.substring(dot)}'),
    );
  }

  switch (raw['type']) {
    case 'clock':
      final hands = raw['hands'];
      if (hands is! List) return null;
      final byName = <String, IconLayer>{};
      for (final hand in hands) {
        if (hand is! Map || hand['unit'] is! String) continue;
        if (layer(hand['file']) case final l?)
          byName[hand['unit'] as String] = l;
      }
      final hour = byName['hour'];
      final minute = byName['minute'];
      final cap = layer(raw['cap']);
      if (hour == null || minute == null || cap == null) return null;
      return ClockHands(
        hour: hour,
        minute: minute,
        second: byName['second'],
        cap: cap,
      );
    case 'calendar':
      final month = _parseText(raw['month']);
      final day = _parseText(raw['day']);
      if (month == null || day == null) return null;
      return CalendarText(month: month, day: day);
    case 'folder-symbol':
      return _parseFolder(raw, dir);
    default:
      return null;
  }
}

double? _number(Object? v, double min, double max) =>
    v is num && v.isFinite && v >= min && v <= max ? v.toDouble() : null;

/// `dynamic` di tipo `folder-symbol`; null se una parte obbligatoria manca o è fuori misura.
FolderSymbol? _parseFolder(Map raw, String dir) {
  final symbol = raw['symbol'];
  if (symbol is! Map) return null;
  final box = symbol['box'];
  if (box is! List || box.length != 4) return null;
  final b = [for (final v in box) _number(v, 0, 1024)];
  if (b.contains(null) || b[2]! <= 0 || b[3]! <= 0) return null;
  final sizes = symbol['sizes'];
  if (sizes is! List || sizes.isEmpty || sizes.length > 8) return null;
  final bands = <SymbolBand>[];
  for (final size in sizes) {
    if (size is! Map) return null;
    final min = _number(size['min_size'], 0, 4096);
    final weight = size['weight'];
    final scale = size['scale'] == null ? 1.0 : _number(size['scale'], 0.25, 4);
    if (min == null || weight is! String || !_weights.contains(weight)) {
      return null;
    }
    if (scale == null) return null;
    bands.add(
      SymbolBand(
        minSize: min,
        weight: weight,
        scale: scale,
        highlight: size['highlight'] != false,
      ),
    );
  }
  bands.sort((a, b) => b.minSize.compareTo(a.minSize));

  final effect = raw['effect'];
  if (effect is! Map) return null;
  SymbolEffect? fx(Object? e) {
    if (e is! Map) return null;
    final color = _parseColor(e['color']);
    final opacity = _number(e['opacity'], 0, 1);
    if (color == null || opacity == null) return null;
    return SymbolEffect(
      color: color,
      opacity: opacity,
      highlight: _parseColor(e['highlight']) ?? const Color(0xFFFFFFFF),
      highlightOpacity: _number(e['highlight_opacity'], 0, 1) ?? 0,
      offset: _number(e['offset'], 0, 64) ?? 0,
    );
  }

  final light = fx(effect['light']);
  if (light == null) return null;
  final dark = fx(effect['dark']) ?? light;

  TintRamp? ramp(Object? r) {
    if (r is! Map) return null;
    final lum = r['luminance'];
    if (lum is! List || lum.length != 2) return null;
    final low = _number(lum[0], 0, 1);
    final high = _number(lum[1], 0, 1);
    final shade = _number(r['shade'], 0, 1);
    final glow = _number(r['glow'], 0, 1);
    if (low == null || high == null || shade == null || glow == null) {
      return null;
    }
    if (high - low < 0.01) return null;
    return TintRamp(low: low, high: high, shade: shade, glow: glow);
  }

  final tint = raw['tint'];
  final labels = <String, Color>{};
  TintRamp? lightTint;
  TintRamp? darkTint;
  if (tint is Map) {
    lightTint = ramp(tint['light']);
    darkTint = ramp(tint['dark']) ?? lightTint;
    final rawLabels = tint['labels'];
    if (rawLabels is Map && rawLabels.length <= 32) {
      for (final MapEntry(:key, :value) in rawLabels.entries) {
        final color = _parseColor(value);
        if (key is String && _labelPattern.hasMatch(key) && color != null) {
          labels[key] = color;
        }
      }
    }
  }

  final folders = <String, String>{};
  final rawFolders = raw['folders'];
  if (rawFolders is Map && rawFolders.length <= 64) {
    for (final MapEntry(:key, :value) in rawFolders.entries) {
      if (key is String &&
          _folderKeyPattern.hasMatch(key) &&
          value is String &&
          _idPattern.hasMatch(value)) {
        folders[key] = value;
      }
    }
  }
  final fallback = raw['fallback'];
  return FolderSymbol(
    box: Rect.fromLTWH(b[0]!, b[1]!, b[2]!, b[3]!),
    bands: bands,
    light: light,
    dark: dark,
    symbolsDir: '${File(dir).parent.parent.path}/symbols',
    lightTint: lightTint,
    darkTint: darkTint,
    labels: labels,
    folders: folders,
    fallback: fallback is String && _idPattern.hasMatch(fallback)
        ? fallback
        : null,
  );
}

IconText? _parseText(Object? raw) {
  if (raw is! Map) return null;
  double? number(Object? v) => v is num && v.isFinite ? v.toDouble() : null;
  final size = number(raw['size']);
  final box = raw['box'];
  final color = _parseColor(raw['color']);
  if (size == null || size <= 0 || size > 1024 || color == null) return null;
  if (box is! List || box.length != 4) return null;
  final b = [for (final v in box) number(v)];
  if (b.contains(null)) return null;
  return IconText(
    size: size,
    weight: (number(raw['weight']) ?? 400).round().clamp(100, 900),
    tracking: number(raw['tracking']) ?? 0,
    color: color,
    darkColor: _parseColor(raw['dark_color']) ?? color,
    box: Rect.fromLTWH(b[0]!, b[1]!, b[2]!, b[3]!),
    maxWidth: number(raw['max_width']),
    upper: raw['case'] == 'upper',
  );
}

Color? _parseColor(Object? raw) {
  if (raw is! String ||
      !RegExp(r'^#[0-9a-fA-F]{6}([0-9a-fA-F]{2})?$').hasMatch(raw)) {
    return null;
  }
  final rgb = int.parse(raw.substring(1, 7), radix: 16);
  final alpha = raw.length == 9 ? int.parse(raw.substring(7), radix: 16) : 0xFF;
  return Color((alpha << 24) | rgb);
}

/// Le cartelle con le icone, dalla più importante: `SILICA_ICONS_DIR` (sviluppo), poi
/// `silica/icons` nelle cartelle dei dati XDG.
List<String> silicaIconDirs(Map<String, String> env) => [
  if (env['SILICA_ICONS_DIR'] case final dir? when dir.isNotEmpty) dir,
  for (final dir in xdgDataDirs(env)) '$dir/silica/icons',
];

final Map<String, LayeredIcon?> _cache = {};

/// L'icona `id` (cartella `apps/<id>` o `files/<id>`), o null.
LayeredIcon? findSilicaIcon(String id, {Map<String, String>? env}) {
  if (!_idPattern.hasMatch(id)) return null;
  final environment = env ?? Platform.environment;
  if (env == null && _cache.containsKey(id)) return _cache[id];
  LayeredIcon? found;
  search:
  for (final base in silicaIconDirs(environment)) {
    for (final group in ['apps', 'files']) {
      final manifest = File('$base/$group/$id/icon.json');
      if (!manifest.existsSync()) continue;
      try {
        found = parseManifest(
          manifest.parent.path,
          manifest.readAsStringSync(),
        );
      } on FileSystemException {
        found = null;
      }
      if (found != null) break search;
    }
  }
  if (env == null) _cache[id] = found;
  return found;
}

/// L'icona di Silica di un'app: solo quella che la voce dichiara con `X-Silica-Icon` (le
/// app di Silica); le altre restano con l'icona del loro tema.
LayeredIcon? silicaIconFor(DesktopEntry entry, {Map<String, String>? env}) {
  final id = entry.silicaIcon;
  return id == null ? null : findSilicaIcon(id, env: env);
}

/// Le cartelle di xdg-user-dirs (`~/.config/user-dirs.dirs`), per chiave: DESKTOP,
/// DOCUMENTS, DOWNLOAD, MUSIC, PICTURES, VIDEOS, TEMPLATES, PUBLICSHARE. Una voce uguale alla
/// home è disattivata (specifica di xdg-user-dirs) e non compare.
Map<String, String> xdgUserDirs(Map<String, String> env) {
  final home = _trimSlash(env['HOME'] ?? '');
  if (home.isEmpty) return const {};
  final config = env['XDG_CONFIG_HOME'] ?? '$home/.config';
  final dirs = <String, String>{};
  try {
    final file = File('$config/user-dirs.dirs');
    if (!file.existsSync() || file.lengthSync() > 64 * 1024) return dirs;
    final line = RegExp(r'^XDG_([A-Z]{1,32})_DIR="([^"]*)"\s*$');
    for (final text in file.readAsLinesSync()) {
      final m = line.firstMatch(text.trim());
      if (m == null) continue;
      var value = m[2]!;
      if (value.startsWith(r'$HOME')) {
        value = '$home${value.substring(5)}';
      } else if (!value.startsWith('/')) {
        continue;
      }
      value = _trimSlash(value);
      if (value.isEmpty || value == home || value.contains('/../')) continue;
      dirs[m[1]!] = value;
    }
  } on FileSystemException {
    return dirs;
  } on FormatException {
    return dirs;
  }
  return dirs;
}

String _trimSlash(String path) {
  var p = path;
  while (p.length > 1 && p.endsWith('/')) {
    p = p.substring(0, p.length - 1);
  }
  return p;
}

/// La chiave della cartella speciale `path` (per `folders` della cartella generica): le chiavi
/// di xdg-user-dirs, poi `HOME`, `APPLICATIONS` (`~/Applications` e `applications` nelle
/// cartelle dei dati XDG) e `LIBRARY` (`~/.local/share`). `SHARED` e `NETWORK` non hanno un
/// percorso: li passa chi conosce la cartella. Null per una cartella qualsiasi.
String? specialFolderKey(String path, {Map<String, String>? env}) {
  final environment = env ?? Platform.environment;
  final p = _trimSlash(path);
  if (p.isEmpty) return null;
  final home = _trimSlash(environment['HOME'] ?? '');
  for (final MapEntry(:key, :value) in xdgUserDirs(environment).entries) {
    if (value == p) return key;
  }
  if (home.isNotEmpty && p == home) return 'HOME';
  final data = xdgDataDirs(environment).map(_trimSlash).toList();
  if ((home.isNotEmpty && p == '$home/Applications') ||
      data.any((d) => p == '$d/applications')) {
    return 'APPLICATIONS';
  }
  if (home.isNotEmpty && p == '$home/.local/share') return 'LIBRARY';
  return null;
}

/// La cartella da disegnare: l'icona (la cartella generica o quella semplice), il simbolo
/// inciso e la tinta.
class FolderIcon {
  const FolderIcon(this.icon, {this.symbol, this.tint});

  final LayeredIcon icon;

  /// Nome di un simbolo di `symbols/` (solo con la cartella generica).
  final String? symbol;
  final Color? tint;
}

/// L'icona della cartella `path`: il simbolo della cartella speciale (o `symbol`, che vince;
/// `key` per le cartelle senza percorso fisso, come `SHARED` e `NETWORK`) e la tinta (`#rrggbb`
/// o il nome di un'etichetta). Senza simbolo né tinta, o se il simbolo non esiste, la cartella
/// semplice (`fallback`). Null se non c'è nessuna icona di cartella.
FolderIcon? resolveFolderIcon(
  String path, {
  String? symbol,
  String? key,
  String? tint,
  Map<String, String>? env,
}) {
  final generic = findSilicaIcon('folder-generic', env: env);
  final dyn = generic?.dynamic;
  if (generic == null || dyn is! FolderSymbol) {
    final plain = findSilicaIcon('folder', env: env);
    return plain == null ? null : FolderIcon(plain);
  }
  final folderKey = key ?? specialFolderKey(path, env: env);
  final name = symbol ?? (folderKey == null ? null : dyn.folders[folderKey]);
  final valid = name != null && dyn.symbolFile(name, 'regular') != null;
  final color = dyn.tintFor(tint);
  if (!valid && color == null) {
    final plain = findSilicaIcon(dyn.fallback ?? 'folder', env: env);
    return FolderIcon(plain ?? generic);
  }
  return FolderIcon(generic, symbol: valid ? name : null, tint: color);
}
