// I pannelli di dettaglio del Centro di controllo (Wi-Fi, Suono, Schermo), da
// plan/reference/controlcenter.md, "pannelli di dettaglio" e "Dal CC al dettaglio".
//
// Un pannello largo 308 pt, 10 pt a sinistra della pila e 6 pt sopra il primo modulo,
// raggio 16, margini di 14 pt; intestazione da 14 pt; righe da 32 pt con un cerchio da 26
// e il testo a 52 pt; separatori di 1 pt; intestazioni di sezione in grassetto grigio
// chiaro. Il modulo cliccato si trasforma nel pannello (un unico rettangolo interpolato,
// curva lenta-veloce-lenta, più lunga se il modulo è più in basso) mentre gli altri svaniscono.
//
// Non fatti: righe dell'hotspot e dell'avviso sulla privacy, le tessere "Modalità scura" e
// "Night Shift" dello Schermo (non ci sono ancora), "Impostazioni…" (nessuna app).

import 'package:flutter/widgets.dart';
import 'package:silica_ui/silica_ui.dart';

import '../system/system_status.dart';
import 'glyphs.dart';

const double detailWidth = 308;

/// Rispetto alla pila: 10 pt a sinistra, 6 pt sopra.
const double detailLeft = -10;
const double detailTop = -6;
const double detailRadius = 16;
const double detailInset = 14;
const double detailTitleSize = 14;
const double detailRow = 32;
const double detailCircle = 26;
const double detailTextLeft = 52;

/// `#7e7f84` su `#6a6b70`: bianco al ~10 %.
const Color detailSeparator = Color(0x1AFFFFFF);
const Color detailSection = Color(0xFFB7B8BE);
const Color detailCircleOff = Color(0xFF8F9095);
const Color detailBlue = Color(0xFF007AFF);

/// Interruttore del Wi-Fi: capsula 54 × 24 (raggio 12) a 9 pt dalla cima e a filo del
/// margine destro, `#0074f8`; manopola bianca 32 × 20 a 2 pt dal bordo.
const Size switchSize = Size(54, 24);
const double switchTop = 9;
const Color switchOn = Color(0xFF0074F8);
const Color switchOff = Color(0x40FFFFFF);

/// Avanzamento della trasformazione (bordo superiore del pannello, Schermo: 338 pt in
/// ~333 ms); per moduli più vicini la durata si accorcia (Wi-Fi, 34 pt: ~215 ms).
const List<(double, double)> detailCurve = [
  (0, 0),
  (13, 0.08),
  (26, 0.19),
  (40, 0.31),
  (66, 0.54),
  (80, 0.63),
  (93, 0.71),
  (120, 0.82),
  (133, 0.86),
  (146, 0.89),
  (173, 0.94),
  (186, 0.95),
  (226, 0.98),
  (253, 0.99),
  (333, 1),
];
const double detailCurveEnd = 333;

/// Opacità degli altri moduli mentre il pannello si apre.
const List<(double, double)> othersFade = [
  (0, 1),
  (13, 0.94),
  (67, 0.56),
  (107, 0.28),
  (147, 0.13),
  (200, 0.05),
  (213, 0.04),
  (240, 0),
];

/// Il ritorno al Centro (Esc) dura di più: ~440 ms la parte principale.
const double detailBackStretch = 1.35;

double _table(List<(double, double)> table, double ms) {
  if (ms <= table.first.$1) return table.first.$2;
  for (var i = 1; i < table.length; i++) {
    final (t1, v1) = table[i];
    if (ms <= t1) {
      final (t0, v0) = table[i - 1];
      return v0 + (v1 - v0) * (ms - t0) / (t1 - t0);
    }
  }
  return table.last.$2;
}

/// Quanto dura la trasformazione per un modulo che sta `distance` pt sotto il pannello.
double detailTimeScale(double distance) =>
    0.65 + 0.35 * (distance / 338).clamp(0.0, 1.0);

/// Avanzamento 0–1 dopo `ms`.
double detailProgress(double ms, double timeScale) =>
    _table(detailCurve, ms / timeScale);

/// Opacità degli altri moduli dopo `ms`.
double othersOpacity(double ms) => _table(othersFade, ms);

Rect lerpRect(Rect a, Rect b, double t) => Rect.fromLTRB(
  a.left + (b.left - a.left) * t,
  a.top + (b.top - a.top) * t,
  a.right + (b.right - a.right) * t,
  a.bottom + (b.bottom - a.bottom) * t,
);

TextStyle detailText(double size, {FontWeight? weight, Color? color}) =>
    TextStyle(
      fontFamily: uiFontFamily,
      package: 'silica_ui',
      fontSize: size,
      fontWeight: weight,
      color: color ?? const Color(0xFFFFFFFF),
      height: 1.2,
    );

/// I dati di un pannello, caricati all'apertura.
class DetailData {
  const DetailData({this.networks = const [], this.outputs = const []});
  final List<WifiNetwork> networks;
  final List<AudioOutput> outputs;
}

/// Un pezzo del pannello in colonna: altezza e widget.
typedef _Piece = (double, Widget);

/// Il contenuto di un pannello e la sua altezza.
class DetailContent {
  DetailContent(this.pieces);
  final List<_Piece> pieces;
  double get height => pieces.fold(0, (h, p) => h + p.$1);

  Widget build() => Column(
    crossAxisAlignment: CrossAxisAlignment.stretch,
    children: [for (final (h, w) in pieces) SizedBox(height: h, child: w)],
  );
}

Widget _separator({double inset = detailInset}) => Center(
  child: Container(
    height: 1,
    margin: EdgeInsets.symmetric(horizontal: inset),
    color: detailSeparator,
  ),
);

/// Intestazione: titolo a sinistra (maiuscole da 16 a 25 pt), eventuale interruttore.
Widget _header(String title, {Widget? trailing}) => Stack(
  children: [
    Positioned(
      left: detailInset,
      top: 12,
      child: Text(
        title,
        style: detailText(detailTitleSize, weight: FontWeight.w600),
      ),
    ),
    if (trailing != null)
      Positioned(
        right: detailInset,
        top: switchTop,
        width: switchSize.width,
        height: switchSize.height,
        child: trailing,
      ),
  ],
);

Widget _section(String title) => Padding(
  padding: const EdgeInsets.only(left: detailInset, top: 9),
  child: Text(
    title,
    style: detailText(12.4, weight: FontWeight.w600, color: detailSection),
  ),
);

class DetailSwitch extends StatelessWidget {
  const DetailSwitch({super.key, required this.on, required this.onChanged});
  final bool on;
  final ValueChanged<bool> onChanged;

  @override
  Widget build(BuildContext context) => GestureDetector(
    onTap: () => onChanged(!on),
    child: Container(
      decoration: BoxDecoration(
        color: on ? switchOn : switchOff,
        borderRadius: BorderRadius.circular(switchSize.height / 2),
      ),
      alignment: on ? Alignment.centerRight : Alignment.centerLeft,
      padding: const EdgeInsets.all(2),
      child: Container(
        width: 32,
        height: 20,
        decoration: BoxDecoration(
          color: const Color(0xFFFFFFFF),
          borderRadius: BorderRadius.circular(10),
        ),
      ),
    ),
  );
}

/// Una riga: cerchio da 26 con il glifo, testo a 52 pt, accessori a destra.
class DetailRow extends StatelessWidget {
  const DetailRow({
    super.key,
    required this.glyph,
    required this.label,
    required this.active,
    this.trailing = const [],
    this.onTap,
  });

  final GlyphKind glyph;
  final String label;
  final bool active;
  final List<Widget> trailing;
  final VoidCallback? onTap;

  @override
  Widget build(BuildContext context) => GestureDetector(
    onTap: onTap,
    behavior: HitTestBehavior.opaque,
    child: Row(
      children: [
        const SizedBox(width: detailInset),
        Container(
          width: detailCircle,
          height: detailCircle,
          decoration: BoxDecoration(
            shape: BoxShape.circle,
            color: active ? const Color(0xFFFFFFFF) : detailCircleOff,
          ),
          child: Center(
            child: Glyph(
              glyph,
              color: active ? detailBlue : const Color(0xFFFFFFFF),
              size: 14,
            ),
          ),
        ),
        const SizedBox(width: detailTextLeft - detailInset - detailCircle),
        Expanded(
          child: Text(
            label,
            maxLines: 1,
            overflow: TextOverflow.ellipsis,
            style: detailText(13),
          ),
        ),
        ...trailing,
        const SizedBox(width: detailInset),
      ],
    ),
  );
}

/// Lucchetto delle reti protette (`#b7b8bd`).
class _Lock extends StatelessWidget {
  const _Lock();

  @override
  Widget build(BuildContext context) => const SizedBox(
    width: 16,
    height: 14,
    child: CustomPaint(painter: _LockPainter()),
  );
}

class _LockPainter extends CustomPainter {
  const _LockPainter();

  @override
  void paint(Canvas canvas, Size size) {
    final paint = Paint()..color = detailSection;
    canvas.drawRRect(
      RRect.fromRectAndRadius(
        Rect.fromLTWH(4, 6, 8, 7),
        const Radius.circular(1.5),
      ),
      paint,
    );
    canvas.drawArc(
      Rect.fromLTWH(5.5, 1.5, 5, 8),
      3.14159,
      3.14159,
      false,
      paint
        ..style = PaintingStyle.stroke
        ..strokeWidth = 1.5,
    );
  }

  @override
  bool shouldRepaint(_LockPainter old) => false;
}

/// Freccia ">" (Altre reti, passaggio del puntatore sui moduli).
class Chevron extends StatelessWidget {
  const Chevron({super.key, this.color = detailSection, this.down = false});
  final Color color;
  final bool down;

  @override
  Widget build(BuildContext context) => SizedBox(
    width: 8,
    height: 12,
    child: CustomPaint(painter: _ChevronPainter(color, down)),
  );
}

class _ChevronPainter extends CustomPainter {
  const _ChevronPainter(this.color, this.down);
  final Color color;
  final bool down;

  @override
  void paint(Canvas canvas, Size size) {
    final paint = Paint()
      ..color = color
      ..style = PaintingStyle.stroke
      ..strokeWidth = 1.8
      ..strokeCap = StrokeCap.round
      ..strokeJoin = StrokeJoin.round;
    final path = down
        ? (Path()
            ..moveTo(1, 4)
            ..lineTo(4, 7.5)
            ..lineTo(7, 4))
        : (Path()
            ..moveTo(2, 1.5)
            ..lineTo(6, 6)
            ..lineTo(2, 10.5));
    canvas.drawPath(path, paint);
  }

  @override
  bool shouldRepaint(_ChevronPainter old) =>
      old.color != color || old.down != down;
}

/// Wi-Fi: interruttore, reti conosciute, "Altre reti" espandibile.
DetailContent wifiDetail({
  required SystemSnapshot s,
  required List<WifiNetwork> networks,
  required bool othersOpen,
  required ValueChanged<bool> onSwitch,
  required ValueChanged<String> onConnect,
  required VoidCallback onToggleOthers,
}) {
  final pieces = <_Piece>[
    (
      60,
      _header(
        'Wi-Fi',
        trailing: DetailSwitch(on: s.wifiEnabled, onChanged: onSwitch),
      ),
    ),
  ];
  if (!s.wifiEnabled) return DetailContent(pieces);
  DetailRow row(WifiNetwork n) => DetailRow(
    glyph: GlyphKind.wifi,
    label: n.ssid,
    active: n.active,
    trailing: [if (n.secure) const _Lock()],
    onTap: n.known && !n.active ? () => onConnect(n.ssid) : null,
  );
  final known = networks.where((n) => n.known || n.active).toList();
  final others = networks.where((n) => !n.known && !n.active).toList();
  pieces.add((1, _separator()));
  if (known.isNotEmpty) {
    pieces.add((
      27,
      _section(known.length == 1 ? 'Rete conosciuta' : 'Reti conosciute'),
    ));
    for (final n in known) {
      pieces.add((detailRow, row(n)));
    }
    pieces
      ..add((5, const SizedBox()))
      ..add((1, _separator()));
  }
  pieces.add((
    28,
    GestureDetector(
      onTap: onToggleOthers,
      behavior: HitTestBehavior.opaque,
      child: Padding(
        padding: const EdgeInsets.only(
          left: detailInset,
          right: detailInset + 2,
          top: 9,
        ),
        child: Row(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Expanded(
              child: Text(
                'Altre reti',
                style: detailText(
                  12.4,
                  weight: FontWeight.w600,
                  color: detailSection,
                ),
              ),
            ),
            Chevron(down: othersOpen),
          ],
        ),
      ),
    ),
  ));
  if (othersOpen) {
    for (final n in others) {
      pieces.add((detailRow, row(n)));
    }
  }
  pieces.add((10, const SizedBox()));
  return DetailContent(pieces);
}

/// Una traccia di cursore nel pannello: 4 pt, da 34 a 265 pt dal bordo.
Widget _detailSlider({
  required double? value,
  required ValueChanged<double> onChanged,
  required Widget Function(double?) track,
}) {
  const left = 34.0;
  const length = 231.0;
  void set(Offset local) =>
      onChanged(((local.dx - left - 10) / (length - 20)).clamp(0.0, 1.0));
  return GestureDetector(
    behavior: HitTestBehavior.opaque,
    onTapDown: (d) => set(d.localPosition),
    onHorizontalDragUpdate: (d) => set(d.localPosition),
    child: Stack(
      children: [
        Positioned(
          left: left,
          width: length,
          top: 6,
          height: 4,
          child: track(value),
        ),
      ],
    ),
  );
}

/// Suono: volume e uscite.
DetailContent soundDetail({
  required SystemSnapshot s,
  required List<AudioOutput> outputs,
  required ValueChanged<double> onVolume,
  required ValueChanged<int> onOutput,
  required Widget Function(double?) track,
}) {
  final pieces = <_Piece>[
    (38, _header('Suono')),
    (
      16,
      _detailSlider(
        value: s.audioAvailable ? (s.muted ? 0 : s.volume) : null,
        onChanged: onVolume,
        track: track,
      ),
    ),
    (10, const SizedBox()),
    (1, _separator()),
  ];
  if (outputs.isNotEmpty) {
    pieces.add((27, _section('Uscita')));
    for (final o in outputs) {
      pieces.add((
        detailRow,
        DetailRow(
          glyph: GlyphKind.speakerSmall,
          label: o.name,
          active: o.isDefault,
          onTap: o.isDefault ? null : () => onOutput(o.id),
        ),
      ));
    }
    pieces.add((5, const SizedBox()));
  }
  pieces.add((10, const SizedBox()));
  return DetailContent(pieces);
}

/// Schermo: luminosità.
DetailContent displayDetail({
  required SystemSnapshot s,
  required ValueChanged<double> onBrightness,
  required Widget Function(double?) track,
}) => DetailContent([
  (43, _header('Schermo')),
  (
    16,
    _detailSlider(
      value: s.brightnessAvailable ? s.brightness : null,
      onChanged: onBrightness,
      track: track,
    ),
  ),
  (13, const SizedBox()),
]);
