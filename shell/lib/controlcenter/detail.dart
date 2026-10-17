// The detail panels of the Control Center (Wi-Fi, Bluetooth, Focus, Display, Sound, Now
// Playing), on the Amalfi kit (`Control Center/Detail/*` and `Row/*`): 312 pt wide Glass/Thick
// panels with a 16 pt radius, 16 pt insets, a 22 pt title (Title 3) with the master switch on
// the header row, 1 pt separators, 32 pt rows with a 28 pt disc and a 16 pt glyph, 20 pt
// section labels and a 24 pt footer item that opens Settings at the panel's pane.
//
// The clicked module turns into the panel (one interpolated rectangle on the measured
// slow-fast-slow curve, controlcenter.md) while the others fade out; the behaviour of each panel is that of
// docs/spec/controlcenter.md. Night Light has no backend yet: its tile shows disabled.

import 'dart:math' as math;

import 'package:flutter/widgets.dart';
import 'package:silica_ui/silica_ui.dart';

import '../l10n/l10n.dart';
import '../system/system_status.dart';
import 'glyphs.dart';
import 'palette.dart';

/// The detail panel is 312 pt wide (components.md: "Control Center detail panels are 312
/// wide"), on the stack's right edge plus 8 pt: it sticks out 14 pt to the left of the stack
/// and 6 pt above the first module.
const double detailWidth = 312;
const double detailLeft = -14;
const double detailTop = -6;
const double detailRadius = 16;

/// Insets and metrics of the kit's rows.
const double detailInset = 16;
const double detailHeader = 48;
const double detailRow = 32;
const double detailDisc = 28;
const double detailGlyph = 16;
const double detailTextLeft = 40;
const double detailSection = 20;
const double detailFooter = 24;
const double detailBottom = 8;

/// Progress of the module -> panel morph (top edge of the Display panel, ~333 ms; measured,
/// controlcenter.md "Module to detail": slow, fast, slow, no overshoot). Modules nearer the panel
/// take less time (Wi-Fi: ~215 ms), see [detailTimeScale].
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

/// Opacity of the other modules while the panel opens (measured, ~240 ms).
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

/// The return to the Center (Esc) takes longer: ~440 ms for the main part (measured 360-500 ms).
const double detailBackStretch = 1.35;

/// How long the morph is simulated, in ms (the return included).
const double detailTotalMs = detailCurveEnd * detailBackStretch;

/// How long the morph lasts for a module [distance] pt below the panel's top edge, relative to
/// the Display module ([displayDistance], 338 pt in the reference): 0.65 for the nearest, 1 for
/// Display. The distance is normalised by the Display's own, so it scales with the Amalfi
/// geometry and the time does not change.
double detailTimeScale(double distance, double displayDistance) =>
    0.65 + 0.35 * (distance / displayDistance).clamp(0.0, 1.0);

/// Progress 0-1 after [ms].
double detailProgress(double ms, double timeScale) =>
    tableAt(detailCurve, ms / timeScale);

/// Opacity of the other modules after [ms].
double othersOpacity(double ms) => tableAt(othersFade, ms);

/// Linear interpolation in a (ms, value) table.
double tableAt(List<(double, double)> table, double ms) {
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

Rect lerpRect(Rect a, Rect b, double t) => Rect.fromLTRB(
  a.left + (b.left - a.left) * t,
  a.top + (b.top - a.top) * t,
  a.right + (b.right - a.right) * t,
  a.bottom + (b.bottom - a.bottom) * t,
);

/// A text of the panels: an Amalfi style, in the palette's [color].
TextStyle detailText(TextStyle base, Color color) =>
    base.copyWith(color: color);

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

_Piece _gap(double height) => (height, const SizedBox());

/// A 1 pt `Line/Separator` inset 16 pt on each side.
_Piece _separator() => (
  1,
  Builder(
    builder: (context) => Padding(
      padding: const EdgeInsets.symmetric(horizontal: detailInset),
      child: ColoredBox(color: CcPalette.of(context).separator),
    ),
  ),
);

/// A separator with its spacing: 4 pt above and 3 pt below (the kit's rhythm). [first]: the
/// one under the header, which has no gap above.
List<_Piece> _rule({bool first = false}) => [
  if (!first) _gap(4),
  _separator(),
  _gap(3),
];

/// Header: the title (Title 3) on the left, an optional switch on the right.
_Piece _header(String title, {Widget? trailing, String? subtitle}) => (
  detailHeader,
  Builder(
    builder: (context) {
      final p = CcPalette.of(context);
      return Stack(
        children: [
          Positioned(
            left: detailInset,
            top: subtitle == null ? 12 : 6,
            right: detailInset + (trailing == null ? 0 : 52),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text(
                  title,
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                  style: AmalfiType.title3(color: p.text),
                ),
                if (subtitle != null)
                  Text(
                    subtitle,
                    maxLines: 1,
                    style: AmalfiType.caption(color: p.secondary),
                  ),
              ],
            ),
          ),
          if (trailing != null)
            Positioned(right: detailInset, top: 12, child: trailing),
        ],
      );
    },
  ),
);

/// A section label (`Row/Section`): Label, `Text/Tertiary`, 20 pt tall.
_Piece _section(String title, {Widget? trailing, VoidCallback? onTap}) => (
  detailSection,
  Builder(
    builder: (context) {
      final p = CcPalette.of(context);
      final row = Padding(
        padding: const EdgeInsets.fromLTRB(detailInset, 2, detailInset, 0),
        child: Row(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Expanded(
              child: Text(
                title,
                maxLines: 1,
                style: AmalfiType.label(
                  color: p.tertiary,
                ).copyWith(fontWeight: FontWeight.w600),
              ),
            ),
            if (trailing != null) trailing,
          ],
        ),
      );
      return onTap == null
          ? row
          : GestureDetector(
              onTap: onTap,
              behavior: HitTestBehavior.opaque,
              child: row,
            );
    },
  ),
);

/// The hover lift under the pointer on a row (`State/Hover Overlay`).
class _Hover extends StatefulWidget {
  const _Hover({required this.child, this.onTap, this.label, this.radius = 8});

  final Widget child;
  final VoidCallback? onTap;
  final String? label;
  final double radius;

  @override
  State<_Hover> createState() => _HoverState();
}

class _HoverState extends State<_Hover> {
  bool _hover = false;

  @override
  Widget build(BuildContext context) {
    final p = CcPalette.of(context);
    final tappable = widget.onTap != null;
    return Semantics(
      button: tappable,
      label: widget.label,
      child: MouseRegion(
        cursor: tappable ? SystemMouseCursors.click : MouseCursor.defer,
        onEnter: (_) => setState(() => _hover = tappable),
        onExit: (_) => setState(() => _hover = false),
        child: GestureDetector(
          onTap: widget.onTap,
          behavior: HitTestBehavior.opaque,
          child: DecoratedBox(
            decoration: BoxDecoration(
              color: _hover ? p.hoverLift : const Color(0x00000000),
              borderRadius: BorderRadius.circular(widget.radius),
            ),
            child: widget.child,
          ),
        ),
      ),
    );
  }
}

/// The footer item that opens Settings at the panel's pane (`Row/Footer`): a separator, then
/// a 24 pt row with the label. Without the Settings app there is no footer.
List<_Piece> _settingsFooter(
  String label,
  VoidCallback? onTap, {
  bool first = false,
}) => [
  if (onTap != null) ...[
    ..._rule(first: first),
    (
      detailFooter,
      Padding(
        padding: const EdgeInsets.symmetric(horizontal: detailInset),
        child: _Hover(
          label: label,
          onTap: onTap,
          child: Padding(
            padding: const EdgeInsets.symmetric(horizontal: 6, vertical: 3),
            child: Builder(
              builder: (context) => Text(
                label,
                maxLines: 1,
                style: AmalfiType.body(color: CcPalette.of(context).text),
              ),
            ),
          ),
        ),
      ),
    ),
  ],
];

/// The master switch of a panel header (Amalfi switch, Regular: 40 × 24).
class DetailSwitch extends StatelessWidget {
  const DetailSwitch({
    super.key,
    required this.on,
    required this.onChanged,
    this.label,
  });
  final bool on;
  final ValueChanged<bool> onChanged;
  final String? label;

  @override
  Widget build(BuildContext context) =>
      AmalfiSwitch(value: on, onChanged: onChanged, semanticLabel: label);
}

/// A row (`Row/Item`, `Row/Network *`): a 28 pt disc with a 16 pt glyph (the accent when
/// active, `Text/Tertiary` otherwise), the label at 40 pt, accessories on the right.
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
  Widget build(BuildContext context) {
    final p = CcPalette.of(context);
    return Padding(
      padding: const EdgeInsets.symmetric(horizontal: detailInset),
      child: _Hover(
        onTap: onTap,
        label: label,
        radius: 8,
        child: Row(
          children: [
            Container(
              width: detailDisc,
              height: detailDisc,
              decoration: BoxDecoration(
                shape: BoxShape.circle,
                color: active ? p.accent : p.discOff,
              ),
              child: Center(
                child: CcGlyph(glyph, color: p.onAccent, size: detailGlyph),
              ),
            ),
            const SizedBox(width: detailTextLeft - detailDisc),
            Expanded(
              child: Text(
                label,
                maxLines: 1,
                overflow: TextOverflow.ellipsis,
                style: AmalfiType.body(color: p.text),
              ),
            ),
            ...trailing,
            const SizedBox(width: 8),
          ],
        ),
      ),
    );
  }
}

/// A glyph of the kit's rows (lock, signal) in `Text/Secondary`.
class _Accessory extends StatelessWidget {
  const _Accessory(this.name, this.size, {this.fallback});

  final String name;
  final double size;
  final AmalfiGlyphData? fallback;

  @override
  Widget build(BuildContext context) {
    final color = CcPalette.of(context).secondary;
    final data = fallback;
    return SizedBox.square(
      dimension: size,
      child: AmalfiSymbol(
        name,
        size: size,
        color: color,
        fallback: data == null
            ? null
            : AmalfiGlyph(data, size: size, color: color),
      ),
    );
  }
}

/// The signal glyph of a network from its strength (0–100).
String signalSymbol(int strength) => strength >= 67
    ? 'wifi'
    : strength >= 34
    ? 'wifi-2'
    : 'wifi-1';

/// A chevron (Other networks: down or up) in `Text/Secondary`.
class Chevron extends StatelessWidget {
  const Chevron({super.key, this.color, this.down = false, this.right = false});

  /// Null: the secondary color.
  final Color? color;
  final bool down;

  /// Pointing right (the hint on a module with a detail).
  final bool right;

  @override
  Widget build(BuildContext context) => AmalfiGlyph(
    right
        ? AmalfiGlyphs.chevronRight
        : down
        ? AmalfiGlyphs.chevronDown
        : AmalfiGlyphs.chevronUp,
    size: 12,
    color: color ?? CcPalette.of(context).secondary,
  );
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
  VoidCallback? onSettings,
}) {
  final pieces = <_Piece>[
    _header(
      'Wi-Fi',
      trailing: DetailSwitch(
        on: s.wifiEnabled,
        onChanged: onSwitch,
        label: 'Wi-Fi',
      ),
    ),
  ];
  if (!s.wifiEnabled) {
    return DetailContent([
      ...pieces,
      ..._settingsFooter(l.ccWifiSettings, onSettings, first: true),
      _gap(detailBottom),
    ]);
  }
  pieces.addAll(_rule(first: true));
  // The privacy warning (SH-CC-14): under the header while the joined network is not
  // secured; a click opens the Wi-Fi settings.
  final insecure = networks.any((n) => n.active && !n.secure);
  if (insecure) {
    pieces.add((
      24,
      GestureDetector(
        onTap: onSettings,
        behavior: HitTestBehavior.opaque,
        child: Padding(
          padding: const EdgeInsets.symmetric(horizontal: detailInset),
          child: Builder(
            builder: (context) {
              final p = CcPalette.of(context);
              return Row(
                children: [
                  Expanded(
                    child: Text(
                      l.ccPrivacyWarning,
                      maxLines: 1,
                      style: AmalfiType.caption(color: p.secondary),
                    ),
                  ),
                  AmalfiGlyph(
                    AmalfiGlyphs.warning,
                    size: 12,
                    color: p.secondary,
                  ),
                ],
              );
            },
          ),
        ),
      ),
    ));
  }
  DetailRow row(WifiNetwork n) => DetailRow(
    glyph: GlyphKind.wifi,
    label: n.ssid,
    active: n.active,
    trailing: [
      if (n.secure) ...[
        const _Accessory('lock', 12, fallback: AmalfiGlyphs.lock),
        const SizedBox(width: 12),
      ],
      _Accessory(signalSymbol(n.strength), 16),
    ],
    onTap: n.known && !n.active ? () => onConnect(n.ssid) : null,
  );
  final known = networks.where((n) => n.known || n.active).toList();
  final others = networks.where((n) => !n.known && !n.active).toList();
  if (known.isNotEmpty) {
    pieces.add(_section(l.knownNetworks(known.length)));
    for (final n in known) {
      pieces.add((detailRow, row(n)));
    }
    pieces.addAll(_rule());
  }
  pieces.add(
    _section(
      l.otherNetworks,
      onTap: onToggleOthers,
      trailing: Chevron(down: othersOpen),
    ),
  );
  if (othersOpen) {
    for (final n in others) {
      pieces.add((detailRow, row(n)));
    }
  }
  pieces.addAll(_settingsFooter(l.ccWifiSettings, onSettings));
  pieces.add(_gap(detailBottom));
  return DetailContent(pieces);
}

/// A slider row (`Row/Slider`): a 16 pt glyph at each end of an [AmalfiSlider]. [value]
/// null: the slider is not adjustable.
class DetailSlider extends StatelessWidget {
  const DetailSlider({
    super.key,
    required this.value,
    required this.onChanged,
    required this.low,
    required this.high,
    this.semanticLabel,
  });

  final double? value;
  final ValueChanged<double> onChanged;
  final GlyphKind low;
  final GlyphKind high;
  final String? semanticLabel;

  @override
  Widget build(BuildContext context) {
    final p = CcPalette.of(context);
    return Padding(
      padding: const EdgeInsets.symmetric(horizontal: detailInset),
      child: Row(
        children: [
          CcGlyph(low, color: p.text, size: 16),
          const SizedBox(width: 8),
          Expanded(
            child: AmalfiSlider(
              value: value ?? 0,
              onChanged: value == null ? null : onChanged,
              semanticLabel: semanticLabel,
            ),
          ),
          const SizedBox(width: 8),
          CcGlyph(high, color: p.text, size: 16),
        ],
      ),
    );
  }
}

/// Sound: volume and outputs.
DetailContent soundDetail({
  required AppLocalizations l,
  required SystemSnapshot s,
  required List<AudioOutput> outputs,
  required ValueChanged<double> onVolume,
  required ValueChanged<int> onOutput,
  VoidCallback? onSettings,
}) {
  final pieces = <_Piece>[
    _header(l.ccSound),
    ..._rule(first: true),
    _gap(4),
    (
      detailRow,
      DetailSlider(
        value: s.audioAvailable ? (s.muted ? 0 : s.volume) : null,
        onChanged: onVolume,
        low: GlyphKind.speakerSmall,
        high: GlyphKind.speakerLarge,
        semanticLabel: l.ccSound,
      ),
    ),
  ];
  if (outputs.isNotEmpty) {
    pieces
      ..add(_gap(4))
      ..addAll(_rule())
      ..add(_section(l.soundOutput));
    for (final o in outputs) {
      pieces.add((
        detailRow,
        DetailRow(
          glyph: GlyphKind.headphones,
          label: o.name,
          active: o.isDefault,
          onTap: o.isDefault ? null : () => onOutput(o.id),
        ),
      ));
    }
  }
  pieces
    ..addAll(_settingsFooter(l.ccSoundSettings, onSettings))
    ..add(_gap(detailBottom));
  return DetailContent(pieces);
}

/// The Dark Mode and Night Light tiles of Display: a 40 pt disc, label and subtitle under it,
/// centred in a 140 pt half of the 280 pt content.
const double displayTileDisc = 40;
const double displayTileWidth = 112;
const List<double> displayTileCentres = [80, 224];

/// Display: the built-in display's brightness, then the Dark Mode and Night Light tiles.
DetailContent displayDetail({
  required AppLocalizations l,
  required SystemSnapshot s,
  required ValueChanged<double> onBrightness,
  required bool dark,
  required VoidCallback onDark,
  required VoidCallback onNightLight,
  VoidCallback? onSettings,
}) {
  Widget tile(
    double centre,
    GlyphKind glyph,
    String label,
    String subtitle,
    bool active,
    VoidCallback? onTap,
  ) => Positioned(
    left: centre - displayTileWidth / 2,
    width: displayTileWidth,
    top: 0,
    bottom: 0,
    child: Builder(
      builder: (context) {
        final p = CcPalette.of(context);
        final body = GestureDetector(
          behavior: HitTestBehavior.opaque,
          onTap: onTap,
          child: Column(
            children: [
              Container(
                width: displayTileDisc,
                height: displayTileDisc,
                decoration: BoxDecoration(
                  shape: BoxShape.circle,
                  color: active ? p.accent : p.discOff,
                ),
                child: Center(
                  child: CcGlyph(glyph, color: p.onAccent, size: 20),
                ),
              ),
              const SizedBox(height: 4),
              Text(
                label,
                maxLines: 1,
                style: AmalfiType.label(
                  color: p.text,
                ).copyWith(fontWeight: FontWeight.w600),
              ),
              Text(
                subtitle,
                maxLines: 1,
                style: AmalfiType.caption(color: p.secondary),
              ),
            ],
          ),
        );
        return onTap == null
            ? Semantics(
                enabled: false,
                child: Opacity(
                  opacity: AmalfiStates.disabledOpacity,
                  child: AbsorbPointer(child: body),
                ),
              )
            : Semantics(button: true, label: label, child: body);
      },
    ),
  );
  return DetailContent([
    _header(l.ccDisplay),
    ..._rule(first: true),
    _section(l.ccBuiltInDisplay),
    (
      detailRow,
      DetailSlider(
        value: s.brightnessAvailable ? s.brightness : null,
        onChanged: onBrightness,
        low: GlyphKind.sunSmall,
        high: GlyphKind.sunLarge,
        semanticLabel: l.ccDisplay,
      ),
    ),
    _gap(4),
    _separator(),
    _gap(7),
    (
      74,
      Stack(
        children: [
          tile(
            displayTileCentres[0],
            GlyphKind.contrast,
            l.ccDarkMode,
            dark ? l.ccOn : l.ccOff,
            dark,
            onDark,
          ),
          tile(
            displayTileCentres[1],
            GlyphKind.sunSmall,
            l.ccNightLight,
            s.nightLight ? l.ccOn : l.ccOff,
            s.nightLight,
            onNightLight,
          ),
        ],
      ),
    ),
    _gap(10),
    ..._settingsFooter(l.ccDisplaySettings, onSettings, first: true),
    _gap(detailBottom),
  ]);
}

/// The glyph of a Bluetooth device from its freedesktop icon name.
GlyphKind deviceGlyph(String icon) => switch (icon) {
  'input-mouse' || 'input-tablet' => GlyphKind.mouse,
  'audio-headset' || 'audio-headphones' || 'audio-card' => GlyphKind.headphones,
  'input-keyboard' => GlyphKind.keyboard,
  _ => GlyphKind.bluetooth,
};

/// Bluetooth: the switch in the header, one 32 pt row per paired device (the connected ones
/// on the accent disc); a click connects or disconnects.
DetailContent bluetoothDetail({
  required AppLocalizations l,
  required SystemSnapshot s,
  required List<BluetoothDevice> devices,
  required ValueChanged<bool> onSwitch,
  required void Function(BluetoothDevice device) onDevice,
  VoidCallback? onSettings,
}) {
  final pieces = <_Piece>[
    _header(
      'Bluetooth',
      trailing: DetailSwitch(
        on: s.bluetoothPowered,
        onChanged: onSwitch,
        label: 'Bluetooth',
      ),
    ),
    ..._rule(first: true),
  ];
  if (s.bluetoothPowered && devices.isNotEmpty) {
    pieces.add(_section(l.ccDevices));
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
  }
  pieces
    ..addAll(_settingsFooter(l.ccBluetoothSettings, onSettings))
    ..add(_gap(detailBottom));
  return DetailContent(pieces);
}

/// Focus: the Do Not Disturb row (accent disc when on) and, when on, the two duration
/// options on a `Surface/Sunken` band.
DetailContent focusDetail({
  required AppLocalizations l,
  required SystemSnapshot s,
  required ValueChanged<bool> onDoNotDisturb,
  required ValueChanged<Duration?> onFor,
  VoidCallback? onSettings,
}) {
  final on = s.doNotDisturb;
  final pieces = <_Piece>[
    _header(l.ccFocus, subtitle: on ? l.ccFocusOn : null),
    ..._rule(first: true),
    _gap(4),
    (
      detailRow,
      DetailRow(
        glyph: GlyphKind.moon,
        label: l.ccDoNotDisturb,
        active: on,
        onTap: () => onDoNotDisturb(!on),
      ),
    ),
  ];
  if (on) {
    Widget option(String label, Duration? duration) => _Hover(
      onTap: () => onFor(duration),
      label: label,
      radius: 6,
      child: Padding(
        padding: const EdgeInsets.symmetric(horizontal: 18, vertical: 0),
        child: Align(
          alignment: Alignment.centerLeft,
          child: Builder(
            builder: (context) => Text(
              label,
              maxLines: 1,
              style: AmalfiType.body(color: CcPalette.of(context).text),
            ),
          ),
        ),
      ),
    );
    pieces
      ..add(_gap(8))
      ..add((
        52,
        Builder(
          builder: (context) => Padding(
            padding: const EdgeInsets.symmetric(horizontal: 10),
            child: DecoratedBox(
              decoration: BoxDecoration(
                color: CcPalette.of(context).band,
                borderRadius: BorderRadius.circular(8),
              ),
              child: Padding(
                padding: const EdgeInsets.symmetric(vertical: 4),
                child: Column(
                  children: [
                    SizedBox(
                      height: 22,
                      child: option(l.focusForAnHour, const Duration(hours: 1)),
                    ),
                    SizedBox(
                      height: 22,
                      child: option(l.focusUntilEvening, null),
                    ),
                  ],
                ),
              ),
            ),
          ),
        ),
      ))
      ..add(_gap(4));
  }
  pieces
    ..addAll(_settingsFooter(l.ccFocusSettings, onSettings))
    ..add(_gap(detailBottom));
  return DetailContent(pieces);
}

/// Clamps a progress that may overshoot to a valid opacity.
double detailOpacity(double progress) => math.max(0, math.min(1, progress));
