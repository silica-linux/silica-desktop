// The detail panels of the Control Center (Wi-Fi, Sound, Display), from
// plan/reference/controlcenter.md, "detail panels" and "From the CC to the detail".
//
// A 308 pt wide panel, 10 pt left of the stack and 6 pt above the first module, radius 16,
// 14 pt margins; 14 pt header; 32 pt rows with a 26 circle and the text at 52 pt; 1 pt
// separators; section headings in light grey bold. The clicked module turns into the panel
// (a single interpolated rectangle, slow-fast-slow curve, longer if the module is lower)
// while the others fade out.
//
// Not done: the hotspot and privacy notice rows, the dark appearance and night light
// tiles of Display (not there yet), the settings item at the bottom (no app).

import 'package:flutter/widgets.dart';
import 'package:silica_ui/silica_ui.dart';

import '../l10n/l10n.dart';
import '../system/system_status.dart';
import 'glyphs.dart';

const double detailWidth = 308;

/// Relative to the stack: 10 pt to the left, 6 pt above.
const double detailLeft = -10;
const double detailTop = -6;
const double detailRadius = 16;
const double detailInset = 14;
const double detailTitleSize = 14;
const double detailRow = 32;
const double detailCircle = 26;
const double detailTextLeft = 52;

/// `#7e7f84` over `#6a6b70`: white at ~10 %.
const Color detailSeparator = Color(0x1AFFFFFF);
const Color detailSection = Color(0xFFB7B8BE);
const Color detailCircleOff = Color(0xFF8F9095);
const Color detailBlue = Color(0xFF007AFF);

/// Wi-Fi switch: 54 × 24 capsule (radius 12) 9 pt from the top and flush with the right
/// margin, `#0074f8`; white 32 × 20 knob 2 pt from the edge.
const Size switchSize = Size(54, 24);
const double switchTop = 9;
const Color switchOn = Color(0xFF0074F8);
const Color switchOff = Color(0x40FFFFFF);

/// Progress of the transformation (top edge of the panel, Display: 338 pt in ~333 ms);
/// for closer modules the duration gets shorter (Wi-Fi, 34 pt: ~215 ms).
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

/// Opacity of the other modules while the panel opens.
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

/// The return to the Center (Esc) takes longer: ~440 ms for the main part.
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

/// How long the transformation lasts for a module `distance` pt below the panel.
double detailTimeScale(double distance) =>
    0.65 + 0.35 * (distance / 338).clamp(0.0, 1.0);

/// Progress 0–1 after `ms`.
double detailProgress(double ms, double timeScale) =>
    _table(detailCurve, ms / timeScale);

/// Opacity of the other modules after `ms`.
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

/// The data of a panel, loaded when it opens.
class DetailData {
  const DetailData({
    this.networks = const [],
    this.outputs = const [],
    this.devices = const [],
  });
  final List<WifiNetwork> networks;
  final List<AudioOutput> outputs;
  final List<BluetoothDevice> devices;
}

/// A piece of the panel in the column: height and widget.
typedef _Piece = (double, Widget);

/// The content of a panel and its height.
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

/// Header: title on the left (capitals from 16 to 25 pt), optional switch.
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

/// A row: a 26 circle with the glyph, text at 52 pt, accessories on the right.
class DetailRow extends StatelessWidget {
  const DetailRow({
    super.key,
    required this.glyph,
    required this.label,
    required this.active,
    this.trailing = const [],
    this.onTap,
    this.activeGlyph = detailBlue,
  });

  final GlyphKind glyph;
  final String label;
  final bool active;
  final List<Widget> trailing;
  final VoidCallback? onTap;

  /// The glyph color on the white disc when active (blue; Focus purple).
  final Color activeGlyph;

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
              color: active ? activeGlyph : const Color(0xFFFFFFFF),
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

/// Lock of protected networks (`#b7b8bd`).
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

/// ">" arrow (Other networks, hovering over the modules).
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

/// Wi-Fi: switch, known networks, expandable "Other networks".
DetailContent wifiDetail({
  required SystemSnapshot s,
  required List<WifiNetwork> networks,
  required AppLocalizations l,
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
    pieces.add((27, _section(l.knownNetworks(known.length))));
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
                l.otherNetworks,
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

/// A slider track in the panel: 4 pt, from 34 to 265 pt from the edge.
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

/// Sound: volume and outputs.
DetailContent soundDetail({
  required AppLocalizations l,
  required SystemSnapshot s,
  required List<AudioOutput> outputs,
  required ValueChanged<double> onVolume,
  required ValueChanged<int> onOutput,
  required Widget Function(double?) track,
}) {
  final pieces = <_Piece>[
    (38, _header(l.ccSound)),
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
    pieces.add((27, _section(l.soundOutput)));
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

/// Display tiles (SH-CC-15): 40 pt circles (`#8f9094`) centered in 138 pt tiles at 9 and
/// 161 from the panel edge, label and subtitle centered under them; the separator 15 pt
/// under the subtitles.
const double displayTileCircle = 40;
const double displayTileWidth = 138;
const List<double> displayTileLefts = [9, 161];

/// Display: brightness, then the Dark Mode and Night Light tiles.
DetailContent displayDetail({
  required AppLocalizations l,
  required SystemSnapshot s,
  required ValueChanged<double> onBrightness,
  required Widget Function(double?) track,
  required bool dark,
  required VoidCallback onDark,
}) {
  Widget tile(
    double left,
    GlyphKind glyph,
    String label,
    String subtitle,
    bool active,
    VoidCallback? onTap,
  ) => Positioned(
    left: left,
    width: displayTileWidth,
    top: 0,
    bottom: 0,
    child: GestureDetector(
      behavior: HitTestBehavior.opaque,
      onTap: onTap,
      child: Column(
        children: [
          Container(
            width: displayTileCircle,
            height: displayTileCircle,
            decoration: BoxDecoration(
              shape: BoxShape.circle,
              color: active ? const Color(0xFFFFFFFF) : detailCircleOff,
            ),
            child: Center(
              child: Glyph(
                glyph,
                color: active ? detailBlue : const Color(0xFFFFFFFF),
                size: 20,
              ),
            ),
          ),
          const SizedBox(height: 3),
          Text(
            label,
            maxLines: 1,
            style: detailText(
              11,
              weight: FontWeight.w600,
              color: onTap == null ? detailSection : null,
            ),
          ),
          Text(
            subtitle,
            maxLines: 1,
            style: detailText(10, color: detailSection),
          ),
        ],
      ),
    ),
  );
  return DetailContent([
    (43, _header(l.ccDisplay)),
    (
      16,
      _detailSlider(
        value: s.brightnessAvailable ? s.brightness : null,
        onChanged: onBrightness,
        track: track,
      ),
    ),
    (14, const SizedBox()),
    (
      69,
      Stack(
        children: [
          tile(
            displayTileLefts[0],
            GlyphKind.contrast,
            l.ccDarkMode,
            dark ? l.ccOn : l.ccOff,
            dark,
            onDark,
          ),
          // Night Light is not there yet: shown disabled.
          tile(
            displayTileLefts[1],
            GlyphKind.sunSmall,
            l.ccNightLight,
            l.ccOff,
            false,
            null,
          ),
        ],
      ),
    ),
    (11, const SizedBox()),
    (1, _separator()),
    (10, const SizedBox()),
  ]);
}

/// The glyph of a Bluetooth device from its freedesktop icon name.
GlyphKind deviceGlyph(String icon) => switch (icon) {
  'input-mouse' || 'input-tablet' => GlyphKind.mouse,
  'audio-headset' || 'audio-headphones' || 'audio-card' => GlyphKind.headphones,
  'input-keyboard' => GlyphKind.keyboard,
  _ => GlyphKind.bluetooth,
};

/// Bluetooth (SH-CC-17): the switch in the header, one 32 pt row per paired device (the
/// connected ones on the white disc); a click connects or disconnects.
DetailContent bluetoothDetail({
  required SystemSnapshot s,
  required List<BluetoothDevice> devices,
  required ValueChanged<bool> onSwitch,
  required void Function(BluetoothDevice device) onDevice,
}) {
  // Header 42 pt: the separator at +42, the first row at +48 (SH-CC-17).
  final pieces = <_Piece>[
    (
      42,
      _header(
        'Bluetooth',
        trailing: DetailSwitch(on: s.bluetoothPowered, onChanged: onSwitch),
      ),
    ),
  ];
  if (!s.bluetoothPowered || devices.isEmpty) return DetailContent(pieces);
  pieces
    ..add((1, _separator()))
    ..add((5, const SizedBox()));
  for (final d in devices) {
    pieces.add((
      detailRow,
      DetailRow(
        glyph: deviceGlyph(d.icon),
        label: d.name,
        active: d.connected,
        onTap: () => onDevice(d),
      ),
    ));
  }
  pieces.add((10, const SizedBox()));
  return DetailContent(pieces);
}

/// Focus (SH-CC-19): header with "Focus on" under it while a Focus is on, the Do Not
/// Disturb row (purple moon on the white disc when on) and, when on, the two duration
/// options on a tinted band (22 pt rows, white at ~14 %).
DetailContent focusDetail({
  required AppLocalizations l,
  required SystemSnapshot s,
  required ValueChanged<bool> onDoNotDisturb,
  required ValueChanged<Duration?> onFor,
}) {
  final on = s.doNotDisturb;
  final pieces = <_Piece>[
    // Header 42 pt with or without the subtitle (separator at +42, SH-CC-19).
    (
      42,
      Padding(
        padding: const EdgeInsets.only(left: detailInset, top: 12),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Text(
              l.ccFocus,
              style: detailText(detailTitleSize, weight: FontWeight.w600),
            ),
            if (on)
              Text(l.ccFocusOn, style: detailText(11, color: detailSection)),
          ],
        ),
      ),
    ),
    (1, _separator()),
    (5, const SizedBox()),
    (
      detailRow,
      DetailRow(
        glyph: GlyphKind.moon,
        label: l.ccDoNotDisturb,
        active: on,
        activeGlyph: focusPurpleGlyph,
        onTap: () => onDoNotDisturb(!on),
      ),
    ),
  ];
  if (on) {
    Widget option(String label, Duration? duration) => GestureDetector(
      behavior: HitTestBehavior.opaque,
      onTap: () => onFor(duration),
      child: Padding(
        padding: const EdgeInsets.only(left: detailTextLeft),
        child: Align(
          alignment: Alignment.centerLeft,
          child: Text(label, style: detailText(13)),
        ),
      ),
    );
    pieces
      ..add((4, const SizedBox()))
      ..add((
        56,
        ColoredBox(
          color: focusBand,
          child: Column(
            children: [
              const SizedBox(height: 6),
              SizedBox(
                height: 22,
                child: option(l.focusForAnHour, const Duration(hours: 1)),
              ),
              SizedBox(height: 22, child: option(l.focusUntilEvening, null)),
            ],
          ),
        ),
      ));
  }
  pieces.add((10, const SizedBox()));
  return DetailContent(pieces);
}

/// The purple of an active Focus: the moon on the white disc (raw `#6f73fb`, the same
/// purple as the privacy pill icon, sRGB `#6e7cff`; SH-CC-06).
const Color focusPurpleGlyph = Color(0xFF6E7CFF);

/// The band of the duration options: `#7e7f84` over `#696b6f` → white at ~14 %.
const Color focusBand = Color(0x24FFFFFF);
