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

enum StackView { fan, grid }

/// Vista automatica per `count` file.
StackView automaticView(int count) =>
    count <= fanMax ? StackView.fan : StackView.grid;

/// I file della cartella, per nome, senza quelli nascosti.
List<FileSystemEntity> stackEntries(String path) {
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
    list.sort((a, b) => naturalCompare(fileName(a.path), fileName(b.path)));
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

/// L'icona di silica-icons per un file o una cartella.
LayeredIcon? fileIcon(FileSystemEntity entry) => entry is Directory
    ? findSilicaIcon('folder')
    : fileIconFor(fileName(entry.path));

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
                  Opacity(opacity: labels, child: _label(label)),
                  const SizedBox(width: 6),
                  SizedBox(
                    width: side,
                    height: side,
                    child: entry == null
                        ? CustomPaint(painter: _OpenFolderPainter())
                        : icon == null
                        ? const SizedBox()
                        : SilicaIcon(
                            LayeredSource(icon),
                            side: side * 0.92,
                            dark: false,
                          ),
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
  Widget _label(String text) => ConstrainedBox(
    constraints: const BoxConstraints(maxWidth: 360),
    child: DecoratedBox(
      decoration: const BoxDecoration(
        color: stackLabelFill,
        borderRadius: BorderRadius.all(Radius.circular(11)),
      ),
      child: Padding(
        padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 4),
        child: Text(
          text,
          maxLines: 1,
          overflow: TextOverflow.ellipsis,
          style: uiText(size: 12, color: stackLabelText),
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
                        color: stackText,
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
    );
  }

  Widget _cell(FileSystemEntity entry) {
    final icon = fileIcon(entry);
    return GestureDetector(
      onTap: () => onOpen(entry),
      child: SizedBox(
        width: gridCell,
        height: gridCell,
        child: Column(
          children: [
            const SizedBox(height: 14),
            SizedBox(
              width: 80,
              height: 80,
              child: icon == null
                  ? null
                  : SilicaIcon(LayeredSource(icon), side: 64, dark: false),
            ),
            const SizedBox(height: 4),
            SizedBox(
              width: gridCell - 12,
              child: Text(
                fileName(entry.path),
                textAlign: TextAlign.center,
                maxLines: 2,
                overflow: TextOverflow.ellipsis,
                style: uiText(size: 12, color: stackText),
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
      child: Column(
        children: [
          const SizedBox(height: 14),
          SizedBox(
            width: 80,
            height: 80,
            child: CustomPaint(painter: _OpenFolderPainter()),
          ),
          const SizedBox(height: 4),
          Text('Apri la cartella', style: uiText(size: 12, color: stackText)),
        ],
      ),
    ),
  );
}
