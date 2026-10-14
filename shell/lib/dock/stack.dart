// Le pile di cartella del Dock (plan/reference/dock-stacks.md, misc.md "Pila di cartella").
//
// Vista automatica: ventaglio fino a 15 file, griglia oltre (la soglia vera è fra 11 e 40,
// non misurata).
// - Ventaglio: dal basso, un elemento ogni 64,6 pt, lato da 65,8 a 82,3 pt salendo, l'arco
//   che piega verso destra con gli spostamenti misurati e le icone ruotate di conseguenza;
//   etichette a sinistra; il primo elemento ha il bordo inferiore 7 pt sopra la zona del
//   Dock; al più 15 file, il sedicesimo posto apre la cartella. Apertura ~240 ms (la
//   colonna esce dall'icona e si allunga, poi le etichette), chiusura ~210 ms.
// - Griglia: pannello con il titolo e celle da 128 pt, ⌈√(n + 1)⌉ colonne, margini 17 / 16
//   ai lati, 32 in alto (titolo), 11 in basso, raggio 25, punta verso l'icona 5 pt sopra il
//   Dock; cresce in scala dalla punta in ~240 ms, si chiude in ~150 ms.
// Un clic su un file chiude la pila e lo apre.

import 'dart:io';
import 'dart:math' as math;

import 'package:flutter/widgets.dart';
import 'package:silica_ui/silica_ui.dart';

import '../faro/faro.dart' show OuterShadow;
import 'stack_settings.dart';

const int fanMax = 15;
const double fanPitch = 64.6;
const double fanFirstSide = 65.8;
const double fanSideStep = 1.1;
const double fanAboveZone = 7;

/// Spostamento a destra del bordo sinistro di ogni elemento rispetto al primo (pt).
const List<double> fanShift = [
  0,
  1.4,
  4.7,
  9.1,
  15.5,
  22.9,
  31.3,
  41.7,
  54.2,
  66.6,
  81.1,
  97.6,
  115.1,
  134.6,
  155.2,
  177.8,
];

/// Centro del primo elemento rispetto al centro dell'icona della pila: 1673,1 + 32,9 − 1703,7.
const double fanFirstOffset = 2.3;

const double gridCell = 128;

/// L'icona nella cella: cartella 92 × 74 a 19 pt dalla cima (riquadro di 92 da y 10).
const double gridIcon = 92;
const double gridIconTop = 10;

/// La tela dell'icona nella cella (silica-icons 0.2: il lato di un file è la tela): 100 pt da
/// y 6, così la cartella (0,922 × 0,734 della tela, cima a 132/1024) torna 92 × 74 a 19 pt.
const double gridIconCanvas = 100;
const double gridCanvasTop = 6;

/// L'etichetta: una riga, maiuscole da y 109 (base a ~118), larga al più ~104.
const double gridLabelBaseline = 118;
const double gridLabelWidth = 104;
const Color gridLabelText = Color(0xFF212122);

/// Il colore viene dal [DefaultTextStyle] della griglia (chiaro o scuro).
TextStyle get gridLabelStyle => const TextStyle(
  fontFamily: uiFontFamily,
  package: 'silica_ui',
  fontSize: 12,
  height: 1,
);

/// La punta verso l'icona: base ~22, alta ~8 (misc.md). Come nei menu, il vertice arrotondato
/// e l'unione morbida allargano la base di ~14 pt.
const double gridTipBase = 22;
const double gridLeft = 17;

/// 16 pt fino al bordo più 1 di filo: 2 colonne fanno 290 pt, 4 ne fanno 546.
const double gridRight = 17;
const double gridTop = 32;
const double gridBottom = 11;
const double gridRadius = 25;
const double gridTipHeight = 8;
const double gridTipAboveDock = 5;

const Duration stackOpen = Duration(milliseconds: 240);
const Duration fanClose = Duration(milliseconds: 210);
const Duration gridClose = Duration(milliseconds: 150);

const Color stackText = Color(0xFF1E1E1F);
const Color stackLabelFill = Color(0xFFBDBEC3);
const Color stackLabelText = Color(0xFF1C1C1D);

/// In scuro le pile non sono misurate (dock-stacks.md, "Non osservato"): *stimati* come le
/// etichette del Dock in scuro (dock.md: corpo `#7f8086`, testo `#f9f9f9`) e testi bianchi
/// sul vetro scuro.
const Color gridLabelTextDark = Color(0xFFF9F9F9);
const Color stackTextDark = Color(0xFFFFFFFF);
const Color stackLabelFillDark = Color(0xFF7F8086);
const Color stackLabelTextDark = Color(0xFFF9F9F9);

bool _dark(BuildContext context) =>
    MediaQuery.maybePlatformBrightnessOf(context) == Brightness.dark;

enum StackView { fan, grid }

/// Vista automatica per `count` file.
StackView automaticView(int count) =>
    count <= fanMax ? StackView.fan : StackView.grid;

/// I file della cartella senza quelli nascosti, nell'ordine scelto: per nome; per data dal più
/// recente (come le pile dei download); per tipo (estensione, poi nome). Su Linux la "data di
/// aggiunta" è il cambio di stato (ctime: spostare o copiare un file nella cartella lo
/// aggiorna) e non c'è una data di creazione affidabile: si usa la modifica.
List<FileSystemEntity> stackEntries(
  String path, {
  StackSort sort = StackSort.name,
}) {
  try {
    final list = Directory(path)
        .listSync()
        .where(
          (e) => !e.uri.pathSegments
              .where((s) => s.isNotEmpty)
              .last
              .startsWith('.'),
        )
        .toList();
    int byName(FileSystemEntity a, FileSystemEntity b) =>
        naturalCompare(fileName(a.path), fileName(b.path));
    final stats = {for (final e in list) e.path: e.statSync()};
    DateTime when(FileSystemEntity e) {
      final stat = stats[e.path];
      if (stat == null) return DateTime(0);
      return sort == StackSort.added ? stat.changed : stat.modified;
    }

    String extension(FileSystemEntity e) {
      if (e is Directory) return '';
      final name = fileName(e.path);
      final dot = name.lastIndexOf('.');
      return dot <= 0 ? '' : name.substring(dot + 1).toLowerCase();
    }

    list.sort(
      (a, b) => switch (sort) {
        StackSort.name => byName(a, b),
        StackSort.kind => () {
          final c = extension(a).compareTo(extension(b));
          return c != 0 ? c : byName(a, b);
        }(),
        _ => () {
          final c = when(b).compareTo(when(a));
          return c != 0 ? c : byName(a, b);
        }(),
      },
    );
    return list;
  } on FileSystemException {
    return const [];
  }
}

/// Confronto "naturale": le cifre contano come numeri (documento-2 prima di documento-10),
/// senza maiuscole.
int naturalCompare(String a, String b) {
  final parts = RegExp(r'\d+|\D+');
  final pa = [for (final m in parts.allMatches(a.toLowerCase())) m[0] ?? ''];
  final pb = [for (final m in parts.allMatches(b.toLowerCase())) m[0] ?? ''];
  for (var i = 0; i < pa.length && i < pb.length; i++) {
    final na = int.tryParse(pa[i]);
    final nb = int.tryParse(pb[i]);
    final c = na != null && nb != null
        ? na.compareTo(nb)
        : pa[i].compareTo(pb[i]);
    if (c != 0) return c;
  }
  return pa.length.compareTo(pb.length);
}

String fileName(String path) {
  final parts = path.split('/').where((p) => p.isNotEmpty);
  return parts.isEmpty ? path : parts.last;
}

/// L'icona di silica-icons per un file o una cartella: le cartelle speciali con il loro
/// simbolo, i file con le regole di silica-icons (tipo MIME, estensione, file generico). Nel
/// Dock niente badge (icon-format.md).
IconSource? fileIcon(FileSystemEntity entry) {
  if (entry is Directory) {
    final folder = resolveFolderIcon(entry.path);
    return folder == null ? null : FolderSource(folder);
  }
  final file = resolveFileIcon(entry.path);
  return file == null ? null : FileSource(file);
}

/// La geometria del ventaglio per l'elemento `i` (dal basso): centro e lato, rispetto al
/// centro dell'icona della pila in x e alla cima della zona del Dock in y; rotazione.
({Offset center, double side, double angle}) fanSlot(int i) {
  final side = fanFirstSide + fanSideStep * i;
  final left = fanShift[i.clamp(0, fanShift.length - 1)];
  final x = fanFirstOffset - fanFirstSide / 2 + left + side / 2;
  // Le cime vanno a passo costante; il primo ha il fondo 7 pt sopra la zona.
  final top = -fanAboveZone - fanFirstSide - fanPitch * i;
  final step = i == 0 ? fanShift[1] : fanShift[i] - fanShift[i - 1];
  return (
    center: Offset(x, top + side / 2),
    side: side,
    angle: math.atan(step / fanPitch),
  );
}

/// Colonne e righe della griglia per `count` file (+1 per aprire la cartella).
(int, int) gridShape(int count) {
  final cells = count + 1;
  final columns = math.sqrt(cells).ceil().clamp(1, 12);
  return (columns, (cells / columns).ceil());
}

Size gridPanelSize(int count, double maxHeight) {
  final (columns, rows) = gridShape(count);
  final height = gridTop + rows * gridCell + gridBottom;
  return Size(
    gridLeft + columns * gridCell + gridRight,
    // Mai un intervallo rovesciato: con poco spazio vince la riga minima.
    height.clamp(
      gridTop + gridCell + gridBottom,
      math.max(gridTop + gridCell + gridBottom, maxHeight),
    ),
  );
}

/// Il ventaglio sopra l'icona: `anchor` è il centro dell'icona in x e la cima della zona
/// del Dock in y; `progress` 0–1 apre e chiude.
class FanStack extends StatelessWidget {
  const FanStack({
    super.key,
    required this.entries,
    required this.anchor,
    required this.progress,
    required this.onOpen,
    required this.onOpenFolder,
  });

  final List<FileSystemEntity> entries;
  final Offset anchor;
  final double progress;
  final ValueChanged<FileSystemEntity> onOpen;
  final VoidCallback onOpenFolder;

  @override
  Widget build(BuildContext context) {
    final shown = entries.take(fanMax).toList();
    final more = entries.length - shown.length;
    final children = <Widget>[];
    final labels = ((progress - 0.7) / 0.3).clamp(0.0, 1.0);
    for (var i = 0; i <= shown.length; i++) {
      final slot = fanSlot(i);
      // La colonna esce dall'icona: il passo cresce con l'avanzamento.
      final center =
          anchor + Offset(slot.center.dx * progress, slot.center.dy * progress);
      final side = slot.side;
      final entry = i < shown.length ? shown[i] : null;
      final label = entry == null
          ? (more > 0 ? 'Altri $more nella cartella' : 'Apri la cartella')
          : fileName(entry.path);
      final icon = entry == null ? null : fileIcon(entry);
      children.add(
        Positioned(
          left: center.dx - side / 2 - 400,
          top: center.dy - side / 2,
          width: side + 400,
          height: side,
          child: Transform.rotate(
            angle: slot.angle * progress,
            alignment: Alignment(1 - side / (side + 400), 0),
            child: GestureDetector(
              behavior: HitTestBehavior.translucent,
              onTap: () => entry == null ? onOpenFolder() : onOpen(entry),
              child: Row(
                mainAxisAlignment: MainAxisAlignment.end,
                children: [
                  Opacity(
                    opacity: labels,
                    child: _label(_dark(context), label),
                  ),
                  const SizedBox(width: 6),
                  SizedBox(
                    width: side,
                    height: side,
                    child: entry == null
                        ? CustomPaint(painter: _OpenFolderPainter())
                        : icon == null
                        ? const SizedBox()
                        : SilicaIcon(icon, side: side, dark: false),
                  ),
                ],
              ),
            ),
          ),
        ),
      );
    }
    return Opacity(
      opacity: progress.clamp(0.0, 1.0),
      child: Stack(clipBehavior: Clip.none, children: children),
    );
  }

  /// Etichetta larga quanto il testo (al più 360 pt), come quelle del Dock.
  Widget _label(bool dark, String text) => ConstrainedBox(
    constraints: const BoxConstraints(maxWidth: 360),
    child: DecoratedBox(
      decoration: BoxDecoration(
        color: dark ? stackLabelFillDark : stackLabelFill,
        borderRadius: const BorderRadius.all(Radius.circular(11)),
      ),
      child: Padding(
        padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 4),
        child: Text(
          text,
          maxLines: 1,
          overflow: TextOverflow.ellipsis,
          style: uiText(
            size: 12,
            color: dark ? stackLabelTextDark : stackLabelText,
          ),
        ),
      ),
    ),
  );
}

/// "Apri la cartella": una freccia in un cerchio.
class _OpenFolderPainter extends CustomPainter {
  @override
  void paint(Canvas canvas, Size size) {
    final c = size.center(Offset.zero);
    final r = size.shortestSide * 0.32;
    canvas.drawCircle(c, r, Paint()..color = const Color(0xFFFFFFFF));
    final paint = Paint()
      ..color = const Color(0xFF3A3A3C)
      ..style = PaintingStyle.stroke
      ..strokeWidth = r * 0.14
      ..strokeCap = StrokeCap.round;
    canvas
      ..drawLine(
        c + Offset(-r * 0.35, r * 0.35),
        c + Offset(r * 0.35, -r * 0.35),
        paint,
      )
      ..drawLine(
        c + Offset(-r * 0.05, -r * 0.35),
        c + Offset(r * 0.35, -r * 0.35),
        paint,
      )
      ..drawLine(
        c + Offset(r * 0.35, -r * 0.35),
        c + Offset(r * 0.35, r * 0.05),
        paint,
      );
  }

  @override
  bool shouldRepaint(_OpenFolderPainter old) => false;
}

/// La griglia: un pannello con il titolo, sopra l'icona con la punta verso di lei.
/// `tip` è il vertice della punta (5 pt sopra il Dock).
class GridStack extends StatelessWidget {
  const GridStack({
    super.key,
    required this.title,
    required this.entries,
    required this.rect,
    required this.tipX,
    required this.progress,
    required this.onOpen,
    required this.onOpenFolder,
  });

  final String title;
  final List<FileSystemEntity> entries;

  /// Il corpo del pannello nell'area.
  final Rect rect;

  /// Il vertice della punta, dal bordo sinistro del pannello.
  final double tipX;
  final double progress;
  final ValueChanged<FileSystemEntity> onOpen;
  final VoidCallback onOpenFolder;

  @override
  Widget build(BuildContext context) {
    final (columns, _) = gridShape(entries.length);
    final p = progress.clamp(0.0, 1.0);
    return Positioned.fromRect(
      rect: rect,
      child: Transform.scale(
        scale: 0.2 + 0.8 * p,
        alignment: Alignment(
          (tipX / rect.width) * 2 - 1,
          1 + 2 * gridTipHeight / rect.height,
        ),
        child: DefaultTextStyle.merge(
          style: TextStyle(
            color: _dark(context) ? gridLabelTextDark : gridLabelText,
          ),
          child: Opacity(
            opacity: p,
            child: OuterShadow(
              radius: gridRadius,
              blurRadius: 30,
              offset: const Offset(0, 8),
              child: LiquidGlass(
                radius: gridRadius,
                material: GlassMaterial.thick,
                interactive: false,
                tip: GlassTip(
                  x: tipX,
                  width: gridTipBase - 14,
                  height: gridTipHeight + 2,
                ),
                child: Stack(
                  children: [
                    Positioned(
                      left: 0,
                      right: 0,
                      top: 9,
                      child: Text(
                        title,
                        textAlign: TextAlign.center,
                        style: TextStyle(
                          fontFamily: uiFontFamily,
                          package: 'silica_ui',
                          fontSize: 13,
                          fontWeight: FontWeight.w600,
                          color: _dark(context) ? stackTextDark : stackText,
                        ),
                      ),
                    ),
                    Positioned(
                      left: gridLeft,
                      right: gridRight,
                      top: gridTop,
                      bottom: gridBottom,
                      child: SingleChildScrollView(
                        child: Wrap(
                          children: [
                            for (final entry in entries) _cell(entry),
                            _openCell(columns),
                          ],
                        ),
                      ),
                    ),
                  ],
                ),
              ),
            ),
          ),
        ),
      ),
    );
  }

  /// Una cella da 128 (misc.md › Pila di cartella): l'icona di ~92 centrata con la cima della
  /// cartella a 19 pt (riquadro da y 10), il nome su una riga troncato al centro con le
  /// maiuscole da y 109, `#212122`.
  Widget _cell(FileSystemEntity entry) {
    final icon = fileIcon(entry);
    return GestureDetector(
      onTap: () => onOpen(entry),
      child: SizedBox(
        width: gridCell,
        height: gridCell,
        child: Stack(
          children: [
            Positioned(
              key: ValueKey('icona ${entry.path}'),
              left: (gridCell - gridIconCanvas) / 2,
              top: gridCanvasTop,
              width: gridIconCanvas,
              height: gridIconCanvas,
              child: icon == null
                  ? const SizedBox.shrink()
                  : SilicaIcon(icon, side: gridIconCanvas, dark: false),
            ),
            Positioned(
              left: (gridCell - gridLabelWidth) / 2,
              width: gridLabelWidth,
              top: 0,
              child: Baseline(
                baseline: gridLabelBaseline,
                baselineType: TextBaseline.alphabetic,
                child: SizedBox(
                  width: gridLabelWidth,
                  child: Text(
                    middleEllipsis(
                      fileName(entry.path),
                      gridLabelStyle,
                      gridLabelWidth,
                    ),
                    textAlign: TextAlign.center,
                    maxLines: 1,
                    softWrap: false,
                    style: gridLabelStyle,
                  ),
                ),
              ),
            ),
          ],
        ),
      ),
    );
  }

  Widget _openCell(int columns) => GestureDetector(
    onTap: onOpenFolder,
    child: SizedBox(
      width: gridCell,
      height: gridCell,
      child: Stack(
        children: [
          Positioned(
            left: (gridCell - gridIcon) / 2,
            top: gridIconTop,
            width: gridIcon,
            height: gridIcon,
            child: CustomPaint(painter: _OpenFolderPainter()),
          ),
          Positioned(
            left: (gridCell - gridLabelWidth) / 2,
            width: gridLabelWidth,
            top: 0,
            child: Baseline(
              baseline: gridLabelBaseline,
              baselineType: TextBaseline.alphabetic,
              child: SizedBox(
                width: gridLabelWidth,
                child: Text(
                  'Apri la cartella',
                  textAlign: TextAlign.center,
                  maxLines: 1,
                  softWrap: false,
                  style: gridLabelStyle,
                ),
              ),
            ),
          ),
        ],
      ),
    ),
  );
}

/// L'icona "pila" di una cartella nel Dock (d06-icon-pila-10-light): il primo file davanti, i
/// due dopo dietro, più piccoli (0,89 e 0,73) e con la cima 4 pt più in alto ciascuno, senza
/// rotazione. Misure su un'icona di 54 pt: foglio davanti 37 × 46, cime a +0, −4, −8.
class PileIcon extends StatelessWidget {
  const PileIcon({super.key, required this.sources, required this.side});

  /// Dal primo (davanti) al terzo.
  final List<IconSource> sources;
  final double side;

  static const List<double> scales = [1, 0.89, 0.73];

  @override
  Widget build(BuildContext context) {
    final unit = side / 54;
    // Il foglio davanti: alto ~46 su 54, appoggiato in basso (2 pt di margine).
    final front = side * 0.88;
    final bottom = side - 2 * unit;
    return SizedBox(
      width: side,
      height: side,
      child: Stack(
        clipBehavior: Clip.none,
        children: [
          for (var i = math.min(sources.length, 3) - 1; i >= 0; i--)
            Positioned(
              left: (side - front * scales[i]) / 2,
              top: bottom - front - 4 * unit * i,
              width: front * scales[i],
              height: front * scales[i],
              // Misurata con il foglio sul corpo delle app (d06-icon-pila-10).
              child: SilicaIcon(
                sources[i],
                side: front * scales[i],
                besideApps: true,
              ),
            ),
        ],
      ),
    );
  }
}

/// Tronca al centro con "…" perché `text` stia in `width` (le etichette della griglia).
String middleEllipsis(String text, TextStyle style, double width) {
  double measure(String s) {
    final painter = TextPainter(
      text: TextSpan(text: s, style: style),
      textDirection: TextDirection.ltr,
      maxLines: 1,
    )..layout();
    final w = painter.width;
    painter.dispose();
    return w;
  }

  if (measure(text) <= width) return text;
  final runes = text.runes.toList();
  var lo = 0;
  var hi = runes.length;
  var best = '…';
  while (lo <= hi) {
    final keep = (lo + hi) ~/ 2;
    final head = (keep + 1) ~/ 2;
    final tail = keep - head;
    final candidate =
        '${String.fromCharCodes(runes.take(head)).trimRight()}…'
        '${String.fromCharCodes(runes.skip(runes.length - tail)).trimLeft()}';
    if (measure(candidate) <= width) {
      best = candidate;
      lo = keep + 1;
    } else {
      hi = keep - 1;
    }
  }
  return best;
}
