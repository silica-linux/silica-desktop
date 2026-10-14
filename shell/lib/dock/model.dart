// Geometria e ingrandimento del Dock, da plan/reference/dock.md (macOS 27, aspetto chiaro,
// cursori delle riprese: dimensione 0,384, ingrandimento 0,429). Tutto in punti logici,
// con l'origine in alto a sinistra della superficie del Dock (larga quanto lo schermo,
// con il bordo inferiore sul bordo dello schermo).

import 'dart:ui' as ui;
import 'package:silica_ui/silica_ui.dart';

/// Passo fra le celle delle app: 63 pt (AX).
const double cellPitch = 63;

/// Larghezza della cella del separatore: 28,35 pt (AX).
const double separatorCell = 28.35;

/// Altezza del fondo: 79 pt pieni (y 1356–1434).
const double backgroundHeight = 79;

/// Distanza fra il fondo e il bordo inferiore dello schermo: 5 pt.
const double bottomGap = 5;

/// Raggio degli angoli del fondo: arco di cerchio di 22 pt.
const double backgroundRadius = 22;

/// Margine fra le celle e il bordo del fondo, ai lati: il fondo a riposo è largo 927 pt
/// contro 910,35 pt di celle (12 app, separatore, 2 elementi): ~8 pt per lato.
const double sidePadding = 8;

/// Lato dell'icona a riposo e distanza dalla cima del fondo: 48 pt, 16 pt.
const double iconSize = 48;
const double iconTop = 16;

/// Quanto sale il bordo inferiore dell'icona ingrandita, per unità di scala oltre 1:
/// +7 pt a ×2,095, +1 pt a ×1,079 (misura a 1 pt).
const double iconLiftPerScale = 6.4;

/// Separatore: riga di 1 pt alta 59 pt, 10 pt di margine nel fondo.
const double separatorHeight = 59;

/// Indicatore delle app aperte: disco di 4 pt, centro 6 pt sopra il bordo inferiore del
/// fondo.
const double indicatorSize = 4;
const double indicatorAbove = 6;

/// Ingrandimento all'impostazione delle riprese: la cella sotto il puntatore passa da 63 a
/// 68 pt (×1,0794). Al massimo del cursore: 132 pt (×2,0952).
const double defaultMagnification = 68 / 63;
const double maxMagnification = 132 / 63;

/// La zona attiva comincia 5 pt sopra il fondo.
const double activeZoneAbove = 5;

/// Profilo dell'ingrandimento f(d) in funzione della distanza d (pt) fra il puntatore e il
/// centro della cella a riposo: tabella misurata, si annulla fra 208 e 224 pt.
const List<(double, double)> magnificationProfile = [
  (0, 1.000),
  (16, 0.991),
  (32, 0.964),
  (48, 0.921),
  (64, 0.861),
  (80, 0.786),
  (96, 0.698),
  (112, 0.597),
  (128, 0.485),
  (144, 0.365),
  (160, 0.240),
  (176, 0.131),
  (192, 0.055),
  (208, 0.011),
  (224, 0.000),
];

/// f(d) per interpolazione lineare della tabella.
double profile(double distance) {
  final d = distance.abs();
  for (var i = 1; i < magnificationProfile.length; i++) {
    final (d1, f1) = magnificationProfile[i];
    if (d <= d1) {
      final (d0, f0) = magnificationProfile[i - 1];
      return f0 + (f1 - f0) * (d - d0) / (d1 - d0);
    }
  }
  return 0;
}

/// Curva dell'entrata e dell'uscita dalla zona attiva (uguali entro 0,01: qui la media
/// delle due), 3 cicli a ingrandimento massimo: avanzamento ogni 10 ms su ~160 ms.
const List<double> enterCurve = [
  0,
  0.001,
  0.017,
  0.067,
  0.112,
  0.206,
  0.294,
  0.386,
  0.481,
  0.578,
  0.672,
  0.759,
  0.837,
  0.902,
  0.955,
  0.988,
  1.0,
];

/// L'avanzamento dell'entrata a una frazione `t` (0–1) della durata.
double enterProgress(double t) {
  final x = t.clamp(0.0, 1.0) * (enterCurve.length - 1);
  final i = x.floor().clamp(0, enterCurve.length - 2);
  return enterCurve[i] + (enterCurve[i + 1] - enterCurve[i]) * (x - i);
}

/// Durata dell'entrata: ~60 ms a 68 pt, ~160 ms a 132 pt; cresce con l'ampiezza (con due
/// soli punti si interpola in linea retta).
Duration enterDuration(double magnification) {
  final span =
      (magnification - defaultMagnification) /
      (maxMagnification - defaultMagnification);
  final ms = 60 + (160 - 60) * span.clamp(0.0, 1.0);
  return Duration(microseconds: (ms * 1000).round());
}

/// Un elemento del Dock: un'app (o cartella, cestino) o il separatore.
enum ItemKind { app, separator }

class Item {
  const Item.app(
    this.label, {
    this.running = false,
    this.iconPath,
    this.layered,
    this.command,
    this.windows = const [],
    this.id,
    this.pinned = false,
    this.folder,
  }) : kind = ItemKind.app,
       minimizedWindow = null,
       thumbnail = null;

  /// Una finestra ridotta (D-08): la sua miniatura con l'icona dell'app nell'angolo.
  const Item.window(
    this.label, {
    required String window,
    this.thumbnail,
    this.iconPath,
    this.layered,
  }) : kind = ItemKind.app,
       running = false,
       command = null,
       windows = const [],
       id = null,
       pinned = false,
       folder = null,
       minimizedWindow = window;
  const Item.separator()
    : kind = ItemKind.separator,
      label = '',
      running = false,
      iconPath = null,
      layered = null,
      command = null,
      windows = const [],
      id = null,
      pinned = false,
      folder = null,
      minimizedWindow = null,
      thumbnail = null;

  final ItemKind kind;
  final String label;
  final bool running;

  /// Il file dell'icona (SVG o PNG); null: segnaposto disegnato in codice.
  final String? iconPath;

  /// L'icona a livelli di silica-icons, preferita a [iconPath].
  final LayeredIcon? layered;

  /// Il comando per avviarla (Exec senza codici di campo), se è un'app.
  final String? command;

  /// Le finestre aperte dell'app (identificatori della lista di Ossido).
  final List<String> windows;

  /// L'id della voce .desktop (null per cartelle e cestino).
  final String? id;

  /// Fissata nel Dock (resta anche chiusa).
  final bool pinned;

  /// Una cartella mostrata come pila (D-06): il suo percorso.
  final String? folder;

  /// La finestra ridotta che questo elemento rappresenta (null per app e cartelle).
  final String? minimizedWindow;

  /// La sua miniatura, se è già arrivata da Ossido.
  final ui.Image? thumbnail;

  /// L'icona da disegnare; null: segnaposto con l'iniziale.
  IconSource? get icon => switch ((layered, iconPath)) {
    (final LayeredIcon l, _) => LayeredSource(l),
    (null, final String path) => FlatSource(path),
    _ => null,
  };

  double get restWidth =>
      kind == ItemKind.separator ? separatorCell : cellPitch;
}

/// La disposizione del Dock in un momento: celle, fondo, icone.
class Layout {
  Layout._(this.cells, this.scales, this.background);

  /// Rettangolo (x, larghezza) di ogni cella, nell'ordine degli elementi.
  final List<(double, double)> cells;

  /// Scala di ogni cella (1 a riposo; il separatore resta 1).
  final List<double> scales;

  /// Il fondo: sinistra, cima, larghezza, altezza.
  final (double, double, double, double) background;

  /// Centro della cella `i`.
  double center(int i) => cells[i].$1 + cells[i].$2 / 2;
}

/// Calcola la disposizione per una superficie larga `width` e alta `height`, con il
/// puntatore a `pointerX` (null: fuori dalla zona attiva) e l'ingrandimento `amount`
/// (0–1, l'avanzamento dell'entrata) fino a `magnification`.
Layout layout(
  List<Item> items, {
  required double width,
  required double height,
  double? pointerX,
  double amount = 0,
  double magnification = defaultMagnification,
}) {
  final restTotal = items.fold<double>(0, (sum, item) => sum + item.restWidth);
  final restLeft = (width - restTotal) / 2;
  // Centri a riposo, da cui si misura la distanza.
  final restCenters = <double>[];
  var x = restLeft;
  for (final item in items) {
    restCenters.add(x + item.restWidth / 2);
    x += item.restWidth;
  }
  final scales = [
    for (var i = 0; i < items.length; i++)
      if (items[i].kind == ItemKind.separator || pointerX == null)
        1.0
      else
        1 + (magnification - 1) * profile(pointerX - restCenters[i]) * amount,
  ];
  // Espansione laterale: il punto sotto il puntatore resta fermo. Ogni cella cede la sua
  // crescita al lato in cui sta rispetto al puntatore; quella sotto il puntatore la
  // divide secondo la posizione del puntatore nella cella a riposo.
  var leftGrowth = 0.0;
  for (var i = 0; i < items.length; i++) {
    final extra = items[i].restWidth * (scales[i] - 1);
    if (extra == 0 || pointerX == null) continue;
    final cellLeft = restCenters[i] - items[i].restWidth / 2;
    final fraction = ((pointerX - cellLeft) / items[i].restWidth).clamp(
      0.0,
      1.0,
    );
    leftGrowth += extra * fraction;
  }
  final cells = <(double, double)>[];
  x = restLeft - leftGrowth;
  for (var i = 0; i < items.length; i++) {
    final w = items[i].restWidth * scales[i];
    cells.add((x, w));
    x += w;
  }
  final first = cells.first.$1;
  final last = cells.last.$1 + cells.last.$2;
  final top = height - bottomGap - backgroundHeight;
  return Layout._(cells, scales, (
    first - sidePadding,
    top,
    last - first + 2 * sidePadding,
    backgroundHeight,
  ));
}

/// Il quadrato dell'icona della cella `i`: sinistra, cima, lato.
(double, double, double) iconRect(Layout l, int i, double height) {
  final side = iconSize * l.scales[i];
  final restBottom = height - bottomGap - backgroundHeight + iconTop + iconSize;
  final bottom = restBottom - iconLiftPerScale * (l.scales[i] - 1);
  return (l.center(i) - side / 2, bottom - side, side);
}

/// Il puntatore è nella zona attiva: dentro il fondo a riposo in orizzontale (x 816 no,
/// x 820 sì con il fondo da 816,5), da 5 pt sopra il fondo in giù.
bool inActiveZone(Layout rest, double x, double y, double height) {
  final top = height - bottomGap - backgroundHeight - activeZoneAbove;
  final (left, _, w, _) = rest.background;
  return y >= top && x >= left && x <= left + w;
}

/// Uscendo verso l'alto la zona resta attiva finché il puntatore è nella cella ingrandita
/// (isteresi, dock.md D-01): fino alla cima della sua icona meno questo margine.
const double activeZoneHold = 6;

/// Resta nella zona: in orizzontale come all'ingresso (la lista a riposo), in verticale fino
/// alla cima della cella ingrandita sotto il puntatore.
bool staysInActiveZone(
  Layout rest,
  Layout current,
  List<Item> items,
  double x,
  double y,
  double height,
) {
  final (left, _, w, _) = rest.background;
  if (x < left || x > left + w) return false;
  if (inActiveZone(rest, x, y, height)) return true;
  for (var i = 0; i < current.cells.length; i++) {
    final (cellLeft, cellWidth) = current.cells[i];
    if (x >= cellLeft && x < cellLeft + cellWidth) {
      final (_, top, _) = iconRect(current, i, height);
      return y >= top - activeZoneHold;
    }
  }
  return false;
}

/// Etichetta (D-12): compare con l'ingresso (piena in ~40 ms), salta subito da un'icona
/// all'altra, all'uscita resta ~40 ms e sfuma in ~250 ms.
const Duration labelAppear = Duration(milliseconds: 40);
const Duration labelHold = Duration(milliseconds: 40);
const Duration labelVanish = Duration(milliseconds: 250);

/// Rimbalzo all'avvio (misc.md): quota in pt sopra la quiete, un salto ogni ~0,7 s, finché
/// l'app non apre una finestra (al più `bounceLimit`).
const List<(double, double)> bounceCurve = [
  (0, 0),
  (27, 3),
  (40, 5),
  (53, 6),
  (80, 8),
  (93, 9),
  (133, 11),
  (160, 13),
  (213, 14),
  (293, 17),
  (440, 14),
  (533, 11),
  (587, 7),
  (613, 6),
  (653, 3),
  (693, 0),
];
const double bouncePeriod = 693;
const Duration bounceLimit = Duration(seconds: 10);

/// Quota del rimbalzo `ms` dopo l'inizio (ripetuto).
double bounceHeight(double ms) {
  if (ms <= 0) return 0;
  final t = ms % bouncePeriod;
  for (var i = 1; i < bounceCurve.length; i++) {
    final (t1, h1) = bounceCurve[i];
    if (t <= t1) {
      final (t0, h0) = bounceCurve[i - 1];
      return h0 + (h1 - h0) * (t - t0) / (t1 - t0);
    }
  }
  return 0;
}

/// La cella sotto il puntatore, se c'è un'app.
/// La cella sotto `x`, separatore compreso.
int? cellAt(Layout l, double x) {
  for (var i = 0; i < l.cells.length; i++) {
    final (left, w) = l.cells[i];
    if (x >= left && x < left + w) return i;
  }
  return null;
}

int? itemAt(Layout l, List<Item> items, double x) {
  for (var i = 0; i < l.cells.length; i++) {
    final (left, w) = l.cells[i];
    if (x >= left && x < left + w) {
      return items[i].kind == ItemKind.app ? i : null;
    }
  }
  return null;
}
