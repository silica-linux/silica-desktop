// The Control Center, from plan/reference/controlcenter.md. In the dark appearance over grey
// it is pixel-identical to light (white texts, same fills): only the glass changes, and
// Ossido draws it.
//
// A stack of separate `regular` glass modules, anchored at the top right: two 140 pt
// columns 12 pt apart, 14 pt from the right edge, the first module at y 42; 64 pt rows at
// a 76 pitch. 140 × 64 capsules (radius 32) with a 36 pt disc 14 pt from the edge and the
// text at 56 pt; the media module 140 × 140 (radius 35); Display and Sound 292 × 64
// (radius ~28) with a knobless slider. A wide soft shadow around the stack; opening and
// closing with the opacities and the scale measured frame by frame.
//
// Modules with a real effect: Wi-Fi, Bluetooth, Do Not Disturb, Display, Sound (from
// silica-daemon), the battery if there is one. Click on the icon disc: on or off; click on
// the text (or on the slider title): the detail panel (`detail.dart`), where there is one;
// with the pointer over a module with a detail the ">" arrow appears. Esc from the detail
// goes back to the Center. There is no "Edit controls" nor the right-click menu yet.
//
// The component keeps running on a full-screen overlay layer: when closed it draws nothing
// and takes neither pointer nor keyboard (silica/layer).

import 'dart:async';
import 'dart:io' show File;

import 'package:flutter/foundation.dart';
import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';
import 'package:silica_ui/silica_ui.dart';

import '../l10n/l10n.dart';
import '../system/layer_control.dart';
import '../system/media.dart';
import '../system/shell_channel.dart';
import '../system/system_status.dart';
import 'detail.dart';
import 'glyphs.dart';
import 'hud.dart';

const double stackWidth = 292;
const double columnWidth = 140;
const double gap = 12;
const double rightMargin = 14;
const double stackTop = 42;
const double rowHeight = 64;
const double rowStep = rowHeight + gap;
const double capsuleRadius = 32;
const double mediaRadius = 35;
const double sliderRadius = 28;

/// Capsule disc: 36 pt, 14 pt from the edge; text at 56 pt.
const double disc = 36;
const double discInset = 14;
const double textLeft = 56;

/// Active disc white with a blue glyph, inactive cool grey with a white glyph.
const Color discOn = Color(0xFFFFFFFF);
const Color discOff = Color(0xFF8F9094);
const Color glyphOn = Color(0xFF007AFF);
const Color glyphOff = Color(0xFFFFFFFF);

/// Titles (9 pt capitals → ~12.4 pt) and subtitles (8 pt → ~11 pt), white.
const double titleSize = 12.4;

/// The Wi-Fi status menu: 1 pt below the bar, right edge 9 pt beyond the icon.
const double statusMenuTop = 31;
const double statusMenuRight = 9;
const double subtitleSize = 11;
const Color textColor = Color(0xFFFFFFFF);

/// Sliders: 4 pt track, white fill up to `x0 + 10 + v · (L − 20)`, the rest `#7e7f84`;
/// track from 36 to 255 pt from the module edge, icons at 15 and 260.
const double trackLeft = 36;
const double trackLength = 219;
const double trackHeight = 4;
const double trackCenter = 42;
const Color trackRest = Color(0xFF7E7F84);

/// Stack shadow: −10 levels on the left edge, nothing at ~130 pt, ~−12 between the stack
/// and the right edge of the screen and above: a gaussian of σ ~65 pt of a rectangle going
/// from the stack to the right edge and from the top of the screen to the bottom of the stack.
const Color shadowColor = Color(0x2A000000);
const double shadowSigma = 65;

/// Opacity while opening and closing (ms from the first changed frame; the first point is
/// one frame earlier). controlcenter.md, "Animations".
const List<(double, double)> openOpacity = [
  (0, 0),
  (13, 0.09),
  (27, 0.16),
  (40, 0.25),
  (67, 0.44),
  (80, 0.53),
  (93, 0.64),
  (120, 0.74),
  (133, 0.80),
  (147, 0.84),
  (173, 0.91),
  (187, 0.92),
  (200, 0.94),
  (227, 0.97),
  (240, 0.98),
  (280, 0.99),
  (310, 1),
];

const List<(double, double)> closeOpacity = [
  (0, 1),
  (13, 0.92),
  (27, 0.86),
  (53, 0.61),
  (67, 0.49),
  (80, 0.38),
  (107, 0.24),
  (120, 0.19),
  (133, 0.14),
  (160, 0.08),
  (173, 0.07),
  (187, 0.05),
  (213, 0.02),
  (253, 0.01),
  (293, 0),
];

/// Content scale: 1.0075 → 1 in ~107 ms when opening, 1 → 1.0075 in ~120 ms when closing,
/// around the center of the stack.
const double scaleAway = 1.0075;
const double openScaleMs = 107;
const double closeScaleMs = 120;
const Duration openDuration = Duration(milliseconds: 310);
const Duration closeDuration = Duration(milliseconds: 293);

double tableAt(List<(double, double)> table, double ms) {
  if (ms <= table.first.$1) return table.first.$2;
  for (var i = 1; i < table.length; i++) {
    final (t1, a1) = table[i];
    if (ms <= t1) {
      final (t0, a0) = table[i - 1];
      return a0 + (a1 - a0) * (ms - t0) / (t1 - t0);
    }
  }
  return table.last.$2;
}

/// Slider value for a point `dx` from the left edge of the track.
double sliderValue(double dx, {double length = trackLength}) =>
    ((dx - 10) / (length - 20)).clamp(0.0, 1.0);

/// End of a slider's fill, from the left edge of the track.
double sliderFill(double value, {double length = trackLength}) =>
    10 + value.clamp(0, 1) * (length - 20);

/// The audio outputs circle in the Sound module (controlcenter.md, light over grey): 26 × 26 with its center at
/// +265.5 from the module edge; the track gets 44 pt shorter and the high volume glyph moves
/// with it. White background at 24 % (`#8f9094` over the module grey).
const double outputsShift = 44;
const double outputsCenter = 265.5;
const double outputsSide = 26;
const Color outputsFill = Color(0x3DFFFFFF);

/// A module: rectangle relative to the stack and radius.
@immutable
class ModuleSlot {
  const ModuleSlot(this.rect, this.radius);
  final Rect rect;
  final double radius;

  @override
  bool operator ==(Object other) =>
      other is ModuleSlot && other.rect == rect && other.radius == radius;

  @override
  int get hashCode => Object.hash(rect, radius);
}

/// The layout of the present modules, from the top (coordinates relative to the stack).
Map<String, ModuleSlot> layoutModules(SystemSnapshot s) {
  final slots = <String, ModuleSlot>{};
  const right = columnWidth + gap;
  slots['wifi'] = const ModuleSlot(
    Rect.fromLTWH(0, 0, columnWidth, rowHeight),
    capsuleRadius,
  );
  slots['bluetooth'] = const ModuleSlot(
    Rect.fromLTWH(0, rowStep, columnWidth, rowHeight),
    capsuleRadius,
  );
  slots['media'] = const ModuleSlot(
    Rect.fromLTWH(right, 0, columnWidth, rowHeight * 2 + gap),
    mediaRadius,
  );
  // The row of the reference's Dark Mode, Screen Capture and Focus (INV-A2 grid): two
  // 64 pt round toggles at 0 and 76, the Focus capsule in the right column.
  slots['dark'] = const ModuleSlot(
    Rect.fromLTWH(0, rowStep * 2, rowHeight, rowHeight),
    rowHeight / 2,
  );
  slots['capture'] = const ModuleSlot(
    Rect.fromLTWH(rowStep, rowStep * 2, rowHeight, rowHeight),
    rowHeight / 2,
  );
  slots['dnd'] = const ModuleSlot(
    Rect.fromLTWH(right, rowStep * 2, columnWidth, rowHeight),
    capsuleRadius,
  );
  var y = rowStep * 3;
  if (s.batteryPresent) {
    slots['battery'] = ModuleSlot(
      Rect.fromLTWH(0, y, columnWidth, rowHeight),
      capsuleRadius,
    );
    y += rowStep;
  }
  if (s.brightnessAvailable) {
    slots['display'] = ModuleSlot(
      Rect.fromLTWH(0, y, stackWidth, rowHeight),
      sliderRadius,
    );
    y += rowStep;
  }
  slots['sound'] = ModuleSlot(
    Rect.fromLTWH(0, y, stackWidth, rowHeight),
    sliderRadius,
  );
  return slots;
}

TextStyle _text(double size, {FontWeight? weight}) => TextStyle(
  fontFamily: uiFontFamily,
  package: 'silica_ui',
  fontSize: size,
  fontWeight: weight,
  color: textColor,
  height: 14 / size,
);

class ControlCenter extends StatefulWidget {
  const ControlCenter({
    super.key,
    required this.system,
    this.startOpen = false,
  });

  final SystemSource system;
  final bool startOpen;

  @override
  State<ControlCenter> createState() => _ControlCenterState();
}

/// The modules with a detail panel.
const Set<String> detailModules = {
  'wifi',
  'media',
  'bluetooth',
  'dnd',
  'sound',
  'display',
};

/// Room around the stack for the detail panel, which sticks out to the left and up.
const Offset detailPad = Offset(-detailLeft, -detailTop);

class _ControlCenterState extends State<ControlCenter>
    with TickerProviderStateMixin {
  late final AnimationController _anim;

  /// The open detail (or opening or closing) and its transformation (value = ms).
  String? _detail;
  bool _detailClosing = false;
  late final AnimationController _detailAnim = AnimationController(
    vsync: this,
    upperBound: detailCurveEnd * detailBackStretch,
    value: 0,
    duration: Duration(
      milliseconds: (detailCurveEnd * detailBackStretch).round(),
    ),
  )..addListener(_detailTick);
  DetailData _detailData = const DetailData();
  bool _othersOpen = false;

  /// Do Not Disturb "for an hour" or "until this evening" (provisional: the timer lives in
  /// the shell and does not survive a restart).
  Timer? _dndTimer;
  final FocusNode _focus = FocusNode();
  StreamSubscription<String>? _commands;
  bool _open = false;

  @override
  void initState() {
    super.initState();
    _anim = AnimationController(vsync: this)
      ..addListener(() => setState(() {}));
    widget.system.addListener(_changed);
    _commands = ShellChannel.instance.commands.listen((id) {
      if (id == 'controlcenter.toggle') _toggle();
    });
    ShellChannel.instance.states.addListener(_statesChanged);
    if (widget.startOpen) {
      _show();
    } else {
      LayerControl.inputRegion(const []);
      LayerControl.keyboard('none');
    }
  }

  @override
  void dispose() {
    widget.system.removeListener(_changed);
    ShellChannel.instance.states.removeListener(_statesChanged);
    _commands?.cancel();
    _anim.dispose();
    _dndTimer?.cancel();
    _detailAnim.dispose();
    _focus.dispose();
    _menuFocus.dispose();
    super.dispose();
  }

  void _changed() => setState(() {});

  void _toggle() => _open ? _hide() : _show();

  /// The status menu opened from the bar (Wi-Fi, Focus): the right edge of the icon.
  double? _wifiMenuRight;
  String? _statusMenuKey;
  final FocusNode _menuFocus = FocusNode();

  /// The panels the bar's status items open.
  static const Set<String> statusMenus = {'wifi', 'dnd', 'media'};

  /// The bar announces `statusmenu` = "<panel> <right edge of the icon>" or "".
  void _statesChanged() {
    final value = ShellChannel.instance.states.value['statusmenu'] ?? '';
    final parts = value.split(' ');
    final key = parts.length == 2 && statusMenus.contains(parts[0])
        ? parts[0]
        : null;
    final right = key == null ? null : double.tryParse(parts[1]);
    if (right == _wifiMenuRight && key == _statusMenuKey) return;
    if (key == null || right == null) {
      _closeWifiMenu(announce: false);
    } else {
      _openWifiMenu(key, right);
    }
  }

  Future<void> _openWifiMenu(String key, double right) async {
    if (_open) _hide();
    setState(() {
      _wifiMenuRight = right;
      _statusMenuKey = key;
      _detail = key;
      _detailClosing = false;
      _othersOpen = false;
      _detailData = const DetailData();
    });
    LayerControl.inputRegion(null);
    LayerControl.keyboard('exclusive');
    WidgetsBinding.instance.addPostFrameCallback(
      (_) => _menuFocus.requestFocus(),
    );
    if (key != 'wifi') return;
    final networks = await widget.system.wifiNetworks();
    if (mounted && _wifiMenuRight != null && _statusMenuKey == 'wifi') {
      setState(() => _detailData = DetailData(networks: networks));
    }
  }

  void _closeWifiMenu({bool announce = true}) {
    if (_wifiMenuRight == null) return;
    _statusMenuKey = null;
    setState(() {
      _wifiMenuRight = null;
      _detail = null;
    });
    if (announce) ShellChannel.instance.broadcast('statusmenu', '');
    if (!_open) {
      LayerControl.keyboard('none');
      LayerControl.inputRegion(const []);
    }
  }

  /// The Wi-Fi panel as a status menu (menubar.md): 308 pt, 1 pt below the bar (y 31), the
  /// right edge 9 pt beyond the icon's; it opens in one frame.
  Widget _wifiMenu(SystemSnapshot s, double right) {
    final content = _detailContent(s);
    if (content == null) return const SizedBox.shrink();
    final rect = Rect.fromLTWH(
      right + statusMenuRight - detailWidth,
      statusMenuTop,
      detailWidth,
      content.height,
    );
    return Focus(
      focusNode: _menuFocus,
      onKeyEvent: (node, event) {
        if (event is KeyDownEvent &&
            event.logicalKey == LogicalKeyboardKey.escape) {
          _closeWifiMenu();
          return KeyEventResult.handled;
        }
        return KeyEventResult.ignored;
      },
      child: Stack(
        children: [
          Positioned.fill(
            child: GestureDetector(
              behavior: HitTestBehavior.opaque,
              onTap: _closeWifiMenu,
            ),
          ),
          Positioned.fromRect(
            rect: rect,
            child: DecoratedBox(
              key: ValueKey('${_statusMenuKey ?? ''} status menu'),
              decoration: BoxDecoration(
                borderRadius: BorderRadius.circular(detailRadius),
                boxShadow: const [
                  BoxShadow(
                    color: Color(0x40000000),
                    blurRadius: 30,
                    offset: Offset(0, 8),
                  ),
                ],
              ),
              child: LiquidGlass(
                radius: detailRadius,
                material: GlassMaterial.regular,
                interactive: false,
                child: ClipRRect(
                  borderRadius: BorderRadius.circular(detailRadius),
                  child: content.build(),
                ),
              ),
            ),
          ),
        ],
      ),
    );
  }

  void _show() {
    _closeWifiMenu();
    setState(() => _open = true);
    ShellChannel.instance.broadcast('controlcenter.open', '1');
    LayerControl.inputRegion(null);
    LayerControl.keyboard('exclusive');
    _anim.duration = openDuration;
    _anim.forward(from: 0);
    WidgetsBinding.instance.addPostFrameCallback((_) => _focus.requestFocus());
  }

  void _detailTick() {
    // Once the return is over, the detail disappears.
    if (_detailClosing && _detailProgress() <= 0) {
      _detail = null;
      _detailClosing = false;
    }
    setState(() {});
  }

  Future<void> _openDetail(String key) async {
    setState(() {
      _detail = key;
      _detailClosing = false;
      _othersOpen = false;
      _detailData = const DetailData();
    });
    _detailAnim.forward(from: 0);
    final system = widget.system;
    final data = switch (key) {
      'wifi' => DetailData(networks: await system.wifiNetworks()),
      'sound' => DetailData(outputs: await system.audioOutputs()),
      'bluetooth' => DetailData(devices: await system.bluetoothDevices()),
      _ => const DetailData(),
    };
    if (mounted && _detail == key) setState(() => _detailData = data);
  }

  void _closeDetail() {
    if (_detail == null || _detailClosing) return;
    setState(() => _detailClosing = true);
    _detailAnim.forward(from: 0);
  }

  /// How much of module `_detail` has become a panel (0–1).
  double _detailProgress() {
    final key = _detail;
    if (key == null) return 0;
    final slot = layoutModules(widget.system.value)[key];
    final scale = detailTimeScale(slot == null ? 0 : slot.rect.top - detailTop);
    if (_detailClosing) {
      return 1 - detailProgress(_detailAnim.value / detailBackStretch, scale);
    }
    return detailProgress(_detailAnim.value, scale);
  }

  void _hide() {
    _detail = null;
    _detailClosing = false;
    setState(() => _open = false);
    ShellChannel.instance.broadcast('controlcenter.open', '0');
    LayerControl.keyboard('none');
    LayerControl.inputRegion(const []);
    _anim.duration = closeDuration;
    _anim.forward(from: 0);
  }

  @override
  Widget build(BuildContext context) => Stack(
    children: [
      Positioned.fill(child: _center(context)),
      if (_wifiMenuRight case final right?)
        Positioned.fill(child: _wifiMenu(widget.system.value, right)),
      // The volume and brightness HUD: not with the Center open.
      Positioned.fill(
        child: IgnorePointer(
          child: SystemHud(system: widget.system, suppressed: _open),
        ),
      ),
    ],
  );

  Widget _center(BuildContext context) {
    final ms =
        _anim.value * (_open ? openDuration : closeDuration).inMilliseconds;
    final opacity = _open
        ? tableAt(openOpacity, ms)
        : tableAt(closeOpacity, ms);
    if (!_open && (opacity <= 0 || !_anim.isAnimating)) {
      return const SizedBox.expand();
    }
    final scale = _open
        ? 1 + (scaleAway - 1) * (1 - (ms / openScaleMs).clamp(0, 1))
        : 1 + (scaleAway - 1) * (ms / closeScaleMs).clamp(0, 1);
    final snapshot = widget.system.value;
    final slots = layoutModules(snapshot);
    final height = slots.values
        .map((s) => s.rect.bottom)
        .reduce((a, b) => a > b ? a : b);
    // The detail: rectangle and radius between the module and the panel, and the content.
    final progress = _detailProgress();
    final opening = !_detailClosing;
    final othersAlpha = _detail == null
        ? 1.0
        : opening
        ? othersOpacity(_detailAnim.value)
        : (1 - progress).clamp(0.0, 1.0);
    final content = _detailContent(snapshot);
    final from = slots[_detail];
    final (Rect, double, DetailContent)? detail =
        content == null || from == null
        ? null
        : (
            lerpRect(
              _glassRect(from.rect),
              Rect.fromLTWH(detailLeft, detailTop, detailWidth, content.height),
              progress,
            ),
            from.radius + (detailRadius - from.radius) * progress,
            content,
          );
    final totalHeight = detail == null
        ? height
        : (detail.$1.bottom > height ? detail.$1.bottom : height);
    final shadowSlots = detail != null && progress > 0.5
        ? [ModuleSlot(detail.$1, detail.$2)]
        : slots.values.toList();

    return LayoutBuilder(
      builder: (context, constraints) {
        final left = constraints.maxWidth - rightMargin - stackWidth;
        final origin = Offset(left, stackTop);
        return Focus(
          focusNode: _focus,
          onKeyEvent: (node, event) {
            if (event is KeyDownEvent &&
                event.logicalKey == LogicalKeyboardKey.escape) {
              // From the detail back to the Center; from the Center it closes.
              _detail != null && !_detailClosing ? _closeDetail() : _hide();
              return KeyEventResult.handled;
            }
            return KeyEventResult.ignored;
          },
          child: Stack(
            children: [
              Positioned.fill(
                child: GestureDetector(
                  onTap: _open ? _hide : null,
                  behavior: HitTestBehavior.opaque,
                ),
              ),
              Positioned.fill(
                child: Opacity(
                  opacity: opacity,
                  // The shadow changes only with the layout: it is drawn once, then only the
                  // opacity of its layer changes.
                  child: RepaintBoundary(
                    child: CustomPaint(
                      painter: _StackShadow(
                        origin: origin,
                        height: totalHeight,
                        slots: shadowSlots,
                      ),
                    ),
                  ),
                ),
              ),
              Positioned(
                left: left - detailPad.dx,
                top: stackTop - detailPad.dy,
                width: detailWidth,
                height: totalHeight + detailPad.dy,
                child: Transform.scale(
                  scale: scale,
                  child: Opacity(
                    opacity: opacity,
                    child: Stack(
                      clipBehavior: Clip.none,
                      children: [
                        // The glass sticks out 1 pt left and right of the module box (the
                        // dark side line falls outside), not above and below.
                        for (final MapEntry(:key, :value) in slots.entries)
                          if (key != _detail)
                            Positioned.fromRect(
                              rect: _glassRect(value.rect).shift(detailPad),
                              child: Opacity(
                                opacity: othersAlpha,
                                child: LiquidGlass(
                                  radius: value.radius,
                                  material: GlassMaterial.regular,
                                  interactive: false,
                                  child: const SizedBox.expand(),
                                ),
                              ),
                            ),
                        if (detail case (final rect, final radius, _))
                          Positioned.fromRect(
                            rect: rect.shift(detailPad),
                            child: LiquidGlass(
                              radius: radius,
                              material: GlassMaterial.regular,
                              interactive: false,
                              child: const SizedBox.expand(),
                            ),
                          ),
                        for (final MapEntry(:key, :value) in slots.entries)
                          Positioned.fromRect(
                            rect: value.rect.shift(detailPad),
                            child: Opacity(
                              opacity: key == _detail
                                  ? (1 - progress * 3).clamp(0.0, 1.0)
                                  : othersAlpha,
                              child: IgnorePointer(
                                ignoring: _detail != null,
                                child: _module(key, snapshot),
                              ),
                            ),
                          ),
                        if (detail case (
                          final rect,
                          final radius,
                          final content,
                        ))
                          Positioned.fromRect(
                            rect: rect.shift(detailPad),
                            child: Opacity(
                              opacity: ((progress - 0.25) / 0.4).clamp(
                                0.0,
                                1.0,
                              ),
                              child: ClipRRect(
                                borderRadius: BorderRadius.circular(radius),
                                child: OverflowBox(
                                  alignment: Alignment.topLeft,
                                  minWidth: detailWidth,
                                  maxWidth: detailWidth,
                                  minHeight: 0,
                                  maxHeight: double.infinity,
                                  child: content.build(),
                                ),
                              ),
                            ),
                          ),
                      ],
                    ),
                  ),
                ),
              ),
            ],
          ),
        );
      },
    );
  }

  /// The glass of a module: 1 pt more on the left and right.
  Rect _glassRect(Rect r) =>
      Rect.fromLTRB(r.left - 1, r.top, r.right + 1, r.bottom);

  /// The content of the open detail.
  DetailContent? _detailContent(SystemSnapshot s) {
    final system = widget.system;
    Widget track(double? v) => CustomPaint(painter: _Track(v));
    final l = l10nOf(context);
    return switch (_detail) {
      'wifi' => wifiDetail(
        l: l,
        s: s,
        networks: _detailData.networks,
        othersOpen: _othersOpen,
        onSwitch: system.setWifi,
        onConnect: system.connectWifi,
        onToggleOthers: () => setState(() => _othersOpen = !_othersOpen),
      ),
      'sound' => soundDetail(
        l: l,
        s: s,
        outputs: _detailData.outputs,
        onVolume: (v) {
          if (s.muted && v > 0) system.setMuted(false);
          system.setVolume(v);
        },
        onOutput: (id) async {
          await system.setAudioOutput(id);
          final outputs = await system.audioOutputs();
          if (mounted)
            setState(() => _detailData = DetailData(outputs: outputs));
        },
        track: track,
      ),
      'display' => displayDetail(
        l: l,
        s: s,
        onBrightness: system.setBrightness,
        track: track,
        dark: MediaQuery.maybePlatformBrightnessOf(context) == Brightness.dark,
        onDark: () => ShellChannel.instance.execute('system.dark_toggle'),
      ),
      'bluetooth' => bluetoothDetail(
        s: s,
        devices: _detailData.devices,
        onSwitch: system.setBluetooth,
        onDevice: (d) async {
          await system.connectBluetooth(d.address, !d.connected);
          final devices = await system.bluetoothDevices();
          if (mounted && _detail == 'bluetooth') {
            setState(() => _detailData = DetailData(devices: devices));
          }
        },
      ),
      'media' => DetailContent([(nowPlayingHeight, const _NowPlayingDetail())]),
      'dnd' => focusDetail(
        l: l,
        s: s,
        onDoNotDisturb: (on) {
          _dndTimer?.cancel();
          system.setDoNotDisturb(on);
        },
        onFor: (duration) {
          _dndTimer?.cancel();
          _dndTimer = Timer(duration ?? untilEvening(DateTime.now()), () {
            system.setDoNotDisturb(false);
          });
        },
      ),
      _ => null,
    };
  }

  Widget _module(String key, SystemSnapshot s) {
    final system = widget.system;
    final l = l10nOf(context);
    return switch (key) {
      'wifi' => _Capsule(
        glyph: GlyphKind.wifi,
        title: 'Wi-Fi',
        subtitle: !s.wifiAvailable
            ? l.ccUnavailable
            : !s.wifiEnabled
            ? l.ccOff
            : s.wifiSsid.isNotEmpty
            ? s.wifiSsid
            : s.wired
            ? l.ccWired
            : l.ccNotConnected,
        active: s.wifiAvailable && s.wifiEnabled,
        onTap: s.wifiAvailable ? () => system.setWifi(!s.wifiEnabled) : null,
        onOpen: s.wifiAvailable ? () => _openDetail('wifi') : null,
      ),
      'bluetooth' => _Capsule(
        glyph: GlyphKind.bluetooth,
        title: 'Bluetooth',
        subtitle: !s.bluetoothAvailable
            ? l.ccUnavailable
            : s.bluetoothPowered
            ? l.ccOn
            : l.ccOff,
        active: s.bluetoothAvailable && s.bluetoothPowered,
        onTap: s.bluetoothAvailable
            ? () => system.setBluetooth(!s.bluetoothPowered)
            : null,
        onOpen: s.bluetoothAvailable ? () => _openDetail('bluetooth') : null,
      ),
      // Focus: off it is titled "Focus"; on, "Do Not Disturb / On" with a purple moon
      // on the white disc (SH-CC-06).
      'dnd' => _Capsule(
        glyph: GlyphKind.moon,
        title: s.doNotDisturb ? l.ccDoNotDisturb : l.ccFocus,
        subtitle: s.doNotDisturb ? l.ccDoNotDisturbOn : '',
        active: s.doNotDisturb,
        activeGlyph: focusPurpleGlyph,
        onTap: () => system.setDoNotDisturb(!s.doNotDisturb),
        onOpen: () => _openDetail('dnd'),
      ),
      'dark' => _RoundToggle(
        glyph: GlyphKind.contrast,
        label: l.ccDarkMode,
        active:
            MediaQuery.maybePlatformBrightnessOf(context) == Brightness.dark,
        onTap: () => ShellChannel.instance.execute('system.dark_toggle'),
      ),
      'capture' => _RoundToggle(
        glyph: GlyphKind.screenCapture,
        label: l.ccScreenCapture,
        active: false,
        onTap: () {
          _hide();
          ShellChannel.instance.execute('screenshot.toolbar');
        },
      ),
      'battery' => _Capsule(
        glyph: GlyphKind.battery,
        glyphLevel: s.batteryPercent / 100,
        title: l.ccBattery,
        subtitle: s.batteryCharging
            ? l.ccBatteryCharging(s.batteryPercent.round())
            : l.ccBatteryLevel(s.batteryPercent.round()),
        active: false,
      ),
      'media' => _Media(onOpen: () => _openDetail('media')),
      'display' => _SliderModule(
        onOpen: () => _openDetail('display'),
        title: l.ccDisplay,
        value: s.brightness,
        enabled: s.brightnessAvailable,
        low: GlyphKind.sunSmall,
        high: GlyphKind.sunLarge,
        onChanged: system.setBrightness,
      ),
      _ => _SliderModule(
        onOpen: () => _openDetail('sound'),
        onOutputs: () => _openDetail('sound'),
        title: l.ccSound,
        value: s.muted ? 0 : s.volume,
        enabled: s.audioAvailable,
        low: GlyphKind.speakerSmall,
        high: GlyphKind.speakerLarge,
        onChanged: (v) {
          if (s.muted && v > 0) system.setMuted(false);
          system.setVolume(v);
        },
      ),
    };
  }
}

class _Capsule extends StatefulWidget {
  const _Capsule({
    required this.glyph,
    required this.title,
    required this.subtitle,
    required this.active,
    this.onTap,
    this.onOpen,
    this.glyphLevel = 1,
    this.activeGlyph = glyphOn,
  });

  final GlyphKind glyph;

  /// The glyph color on the white disc when on (blue, purple for Focus).
  final Color activeGlyph;
  final double glyphLevel;
  final String title;
  final String subtitle;
  final bool active;

  /// The icon disc: on or off.
  final VoidCallback? onTap;

  /// The text: opens the detail; without a detail the text acts like the disc.
  final VoidCallback? onOpen;

  @override
  State<_Capsule> createState() => _CapsuleState();
}

/// The ">" arrow of modules with a detail, on hover: on the right, above the text (which
/// shortens). Position and size estimated from the capture (CC-01).
const double hoverChevronRight = 16;

class _CapsuleState extends State<_Capsule> {
  bool _hover = false;

  @override
  Widget build(BuildContext context) {
    final widget = this.widget;
    final arrow = _hover && widget.onOpen != null;
    return MouseRegion(
      onEnter: (_) => setState(() => _hover = true),
      onExit: (_) => setState(() => _hover = false),
      child: Stack(
        children: [
          // Text: detail (or, without one, the same effect as the disc).
          Positioned.fill(
            child: GestureDetector(
              onTap: widget.onOpen ?? widget.onTap,
              behavior: HitTestBehavior.opaque,
            ),
          ),
          Positioned(
            left: discInset,
            top: discInset,
            width: disc,
            height: disc,
            child: GestureDetector(
              onTap: widget.onTap,
              child: DecoratedBox(
                decoration: BoxDecoration(
                  shape: BoxShape.circle,
                  color: widget.active ? discOn : discOff,
                ),
                child: Center(
                  child: Glyph(
                    widget.glyph,
                    color: widget.active ? widget.activeGlyph : glyphOff,
                    level: widget.glyphLevel,
                  ),
                ),
              ),
            ),
          ),
          Positioned(
            left: textLeft,
            right: arrow ? hoverChevronRight + 12 : 10,
            top: 0,
            bottom: 0,
            child: IgnorePointer(
              child: Column(
                mainAxisAlignment: MainAxisAlignment.center,
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  // As in the reference long texts wrap ("Full immersion", the name of a
                  // network), at most two lines each.
                  Text(
                    widget.title,
                    maxLines: 2,
                    overflow: TextOverflow.ellipsis,
                    style: _text(titleSize, weight: FontWeight.w600),
                  ),
                  if (widget.subtitle.isNotEmpty)
                    Text(
                      widget.subtitle,
                      maxLines: 2,
                      overflow: TextOverflow.ellipsis,
                      style: _text(subtitleSize),
                    ),
                ],
              ),
            ),
          ),
          if (arrow)
            const Positioned(
              right: hoverChevronRight,
              top: (rowHeight - 12) / 2,
              child: IgnorePointer(child: Chevron(color: textColor)),
            ),
        ],
      ),
    );
  }
}

/// "Until this evening": 19:00 today, or the end of the day once past it (provisional).
Duration untilEvening(DateTime now) {
  final evening = DateTime(now.year, now.month, now.day, 19);
  final end = now.isBefore(evening)
      ? evening
      : DateTime(now.year, now.month, now.day + 1);
  return end.difference(now);
}

/// A 64 pt round toggle (Dark Mode, Screen Capture; SH-CC-09, SH-CC-10): the module itself
/// is the button. Off: the white glyph on the module glass, lighter under the pointer
/// (+9 levels, CC-01); on: the whole module white with a black glyph (SH-CC-03 dark).
class _RoundToggle extends StatefulWidget {
  const _RoundToggle({
    required this.glyph,
    required this.label,
    required this.active,
    required this.onTap,
  });

  final GlyphKind glyph;
  final String label;
  final bool active;
  final VoidCallback onTap;

  @override
  State<_RoundToggle> createState() => _RoundToggleState();
}

class _RoundToggleState extends State<_RoundToggle> {
  bool _hover = false;

  @override
  Widget build(BuildContext context) => Semantics(
    button: true,
    toggled: widget.active,
    label: widget.label,
    child: MouseRegion(
      onEnter: (_) => setState(() => _hover = true),
      onExit: (_) => setState(() => _hover = false),
      child: GestureDetector(
        onTap: widget.onTap,
        child: DecoratedBox(
          decoration: BoxDecoration(
            shape: BoxShape.circle,
            color: widget.active
                ? discOn
                : _hover
                ? const Color(0x12FFFFFF)
                : null,
          ),
          child: Center(
            child: Glyph(
              widget.glyph,
              color: widget.active ? const Color(0xFF000000) : glyphOff,
              size: 24,
            ),
          ),
        ),
      ),
    ),
  );
}

/// The media module (controlcenter.md, light over grey): the 40 × 40 cover 14 pt from the edge, the text from
/// x 15 (capitals of the first line at y 68), the transport buttons centered at x
/// 28.5 / 71 / 110.5 and y 111 (⏮ and ⏭ 22 × 12, ▶ 17 × 20). With nothing playing the buttons
/// are dimmed (white at 33 and 51 %); with an MPRIS player it shows title and artist and
/// controls them.
class _Media extends StatefulWidget {
  const _Media({this.onOpen});

  /// A click on the rest of the module (cover, title): the Now Playing detail.
  final VoidCallback? onOpen;

  @override
  State<_Media> createState() => _MediaState();
}

/// The media module (controlcenter.md › "Media module while playing", CC-04): 40 × 40 cover
/// at (14, 14) with radius ~8 and a light shadow; `#f0f0f1` semibold title and below it the
/// `#b7b8bd` artist (one line each) from (14, 64); three 30 × 30 buttons centered at y 111
/// and x 30 / 70 / 110. Glyphs: active `#b7b8bd` (white ~52 % over the `#696a6f` background),
/// disabled `#9d9ea3` (~35 %); with the pointer on the module everything brightens by ~7
/// levels and the glyph under the pointer goes to `#bebfc4`; pressed it turns white `#f7f7f8`, without a pill.
const double mediaButtonSide = 30;
const List<double> mediaButtonCenters = [30, 70, 110];
const double mediaArtRadius = 8;
const Color mediaTitle = Color(0xE6FFFFFF);
const Color mediaSubtitle = Color(0x85FFFFFF);
const double mediaGlyphActive = 0.52;
const double mediaGlyphDisabled = 0.35;
const double mediaGlyphHover = 0.55;
const double mediaGlyphPressed = 0.96;

/// The module under the pointer: +7 levels on the background (`#6a6b70` → `#717277`).
const Color mediaHover = Color(0x0CFFFFFF);

class _MediaState extends State<_Media> {
  final MediaPlayers _players = MediaPlayers.instance;
  bool _hover = false;
  String? _hovered;
  String? _pressed;

  @override
  void initState() {
    super.initState();
    _players.start();
    _players.addListener(_changed);
  }

  @override
  void dispose() {
    _players.removeListener(_changed);
    super.dispose();
  }

  void _changed() => setState(() {});

  Widget _art(NowPlaying? now) {
    final url = now?.artUrl ?? '';
    final file = url.startsWith('file://')
        ? File(Uri.parse(url).toFilePath())
        : null;
    return DecoratedBox(
      decoration: BoxDecoration(
        borderRadius: BorderRadius.circular(mediaArtRadius),
        boxShadow: const [
          BoxShadow(
            color: Color(0x33000000),
            blurRadius: 3,
            offset: Offset(0, 1),
          ),
        ],
      ),
      child: ClipRRect(
        borderRadius: BorderRadius.circular(mediaArtRadius),
        child: file != null && file.existsSync()
            ? Image.file(file, fit: BoxFit.cover)
            : const ColoredBox(color: Color(0xFF7E7F84)),
      ),
    );
  }

  Widget _button({
    required String key,
    required _Transport transport,
    required double centerX,
    required bool enabled,
    VoidCallback? onTap,
  }) {
    final alpha = !enabled
        ? mediaGlyphDisabled
        : _pressed == key
        ? mediaGlyphPressed
        : _hovered == key
        ? mediaGlyphHover
        : mediaGlyphActive;
    return Positioned(
      left: centerX - mediaButtonSide / 2,
      top: 111 - mediaButtonSide / 2,
      width: mediaButtonSide,
      height: mediaButtonSide,
      child: MouseRegion(
        onEnter: (_) => setState(() => _hovered = key),
        onExit: (_) => setState(() {
          if (_hovered == key) _hovered = null;
        }),
        child: GestureDetector(
          key: ValueKey(key),
          behavior: HitTestBehavior.opaque,
          onTapDown: enabled ? (_) => setState(() => _pressed = key) : null,
          onTapCancel: () => setState(() => _pressed = null),
          onTapUp: (_) => setState(() => _pressed = null),
          onTap: enabled ? onTap : null,
          child: Center(
            child: CustomPaint(
              size: switch (transport) {
                _Transport.play => const Size(17, 20),
                _Transport.pause => const Size(15, 20),
                _ => const Size(22, 13),
              },
              painter: _TransportPainter(
                transport,
                const Color(0xFFFFFFFF).withValues(alpha: alpha),
              ),
            ),
          ),
        ),
      ),
    );
  }

  @override
  Widget build(BuildContext context) {
    final now = _players.now;
    final active = now != null;
    return MouseRegion(
      onEnter: (_) => setState(() => _hover = true),
      onExit: (_) => setState(() => _hover = false),
      child: Stack(
        children: [
          Positioned.fill(
            child: GestureDetector(
              behavior: HitTestBehavior.opaque,
              onTap: active ? widget.onOpen : null,
            ),
          ),
          if (_hover)
            const Positioned.fill(
              child: IgnorePointer(
                child: DecoratedBox(
                  key: ValueKey('media under pointer'),
                  decoration: BoxDecoration(
                    color: mediaHover,
                    borderRadius: BorderRadius.all(
                      Radius.circular(mediaRadius),
                    ),
                  ),
                ),
              ),
            ),
          Positioned(
            left: 14,
            top: 14,
            width: 40,
            height: 40,
            child: IgnorePointer(child: _art(now)),
          ),
          Positioned(
            left: 14,
            right: 12,
            top: 64,
            child: IgnorePointer(
              child: active
                  ? Column(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: [
                        Text(
                          now.title.isEmpty
                              ? l10nOf(context).mediaUntitled
                              : now.title,
                          maxLines: 1,
                          softWrap: false,
                          overflow: TextOverflow.ellipsis,
                          style: _text(
                            titleSize,
                            weight: FontWeight.w600,
                          ).copyWith(color: mediaTitle),
                        ),
                        if (now.artist.isNotEmpty)
                          Text(
                            now.artist,
                            maxLines: 1,
                            softWrap: false,
                            overflow: TextOverflow.ellipsis,
                            style: _text(
                              subtitleSize,
                            ).copyWith(color: mediaSubtitle),
                          ),
                      ],
                    )
                  : Text(
                      l10nOf(context).mediaNothingPlaying,
                      style: _text(
                        titleSize,
                        weight: FontWeight.w600,
                      ).copyWith(height: 15 / titleSize),
                    ),
            ),
          ),
          _button(
            key: 'precedente',
            transport: _Transport.previous,
            centerX: mediaButtonCenters[0],
            enabled: active && now.canPrevious,
            onTap: active ? _players.previous : null,
          ),
          _button(
            key: 'riproduci',
            transport: active && now.playing
                ? _Transport.pause
                : _Transport.play,
            centerX: mediaButtonCenters[1],
            enabled: active,
            onTap: active ? _players.playPause : null,
          ),
          _button(
            key: 'successivo',
            transport: _Transport.next,
            centerX: mediaButtonCenters[2],
            enabled: active && now.canNext,
            onTap: active ? _players.next : null,
          ),
        ],
      ),
    );
  }
}

/// The Now Playing detail (SH-CC-20; controlcenter.md › Now Playing media module): a
/// 308 × 140 panel, the 112 pt cover at (14, 14), title and artist from (140, 15), three
/// 26 pt buttons at y 64 and x 148 / 204 / 260 (pitch 56), the 154 × 12 progress bar at
/// (140, 102) (4 pt track `#232323`, elapsed sRGB `#1482ff`) and the times under it
/// (`#b7b8be`). Cover radius provisional.
const double nowPlayingHeight = 140;
const double nowPlayingArt = 112;
const double nowPlayingArtRadius = 10;
const double nowPlayingTextLeft = 140;
const double nowPlayingButton = 26;
const List<double> nowPlayingButtonLefts = [148, 204, 260];
const double nowPlayingButtonTop = 64;
const Rect nowPlayingProgress = Rect.fromLTWH(140, 102, 154, 12);
const Color nowPlayingTrack = Color(0xFF232323);
const Color nowPlayingElapsed = Color(0xFF1482FF);
const Color nowPlayingTime = Color(0xFFB7B8BE);

/// "01:03", "1:02:03".
String mediaTime(Duration d) {
  String two(int n) => n.toString().padLeft(2, '0');
  final h = d.inHours, m = d.inMinutes % 60, s = d.inSeconds % 60;
  return h > 0 ? '$h:${two(m)}:${two(s)}' : '${two(m)}:${two(s)}';
}

class _NowPlayingDetail extends StatefulWidget {
  const _NowPlayingDetail();

  @override
  State<_NowPlayingDetail> createState() => _NowPlayingDetailState();
}

class _NowPlayingDetailState extends State<_NowPlayingDetail> {
  final MediaPlayers _players = MediaPlayers.instance;
  Timer? _tick;

  @override
  void initState() {
    super.initState();
    _players.addListener(_changed);
    // The position moves on while playing: once a second.
    _tick = Timer.periodic(const Duration(seconds: 1), (_) => _changed());
  }

  @override
  void dispose() {
    _tick?.cancel();
    _players.removeListener(_changed);
    super.dispose();
  }

  void _changed() {
    if (mounted) setState(() {});
  }

  Widget _button(_Transport kind, int index, bool enabled, VoidCallback tap) =>
      Positioned(
        left: nowPlayingButtonLefts[index],
        top: nowPlayingButtonTop,
        width: nowPlayingButton,
        height: nowPlayingButton,
        child: GestureDetector(
          behavior: HitTestBehavior.opaque,
          onTap: enabled ? tap : null,
          child: Center(
            child: CustomPaint(
              size: switch (kind) {
                _Transport.play => const Size(15, 18),
                _Transport.pause => const Size(13, 18),
                _ => const Size(20, 12),
              },
              painter: _TransportPainter(
                kind,
                const Color(0xFFFFFFFF).withValues(
                  alpha: enabled ? mediaGlyphActive : mediaGlyphDisabled,
                ),
              ),
            ),
          ),
        ),
      );

  @override
  Widget build(BuildContext context) {
    final now = _players.now;
    final url = now?.artUrl ?? '';
    final file = url.startsWith('file://')
        ? File(Uri.parse(url).toFilePath())
        : null;
    final length = now?.length ?? Duration.zero;
    final position = now?.positionAt(DateTime.now()) ?? Duration.zero;
    final fraction = length > Duration.zero
        ? (position.inMilliseconds / length.inMilliseconds).clamp(0.0, 1.0)
        : 0.0;
    final timeStyle = _text(10).copyWith(color: nowPlayingTime, height: 1.3);
    return Stack(
      children: [
        Positioned(
          left: 14,
          top: 14,
          width: nowPlayingArt,
          height: nowPlayingArt,
          child: ClipRRect(
            borderRadius: BorderRadius.circular(nowPlayingArtRadius),
            child: file != null && file.existsSync()
                ? Image.file(file, fit: BoxFit.cover)
                : const ColoredBox(color: Color(0xFF7C7D82)),
          ),
        ),
        Positioned(
          left: nowPlayingTextLeft,
          right: 14,
          top: 15,
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Text(
                now == null || now.title.isEmpty
                    ? l10nOf(context).mediaUntitled
                    : now.title,
                maxLines: 1,
                softWrap: false,
                overflow: TextOverflow.ellipsis,
                style: _text(
                  titleSize,
                  weight: FontWeight.w600,
                ).copyWith(color: mediaTitle),
              ),
              if (now != null && now.artist.isNotEmpty)
                Text(
                  now.artist,
                  maxLines: 1,
                  softWrap: false,
                  overflow: TextOverflow.ellipsis,
                  style: _text(subtitleSize).copyWith(color: mediaSubtitle),
                ),
            ],
          ),
        ),
        _button(
          _Transport.previous,
          0,
          now?.canPrevious ?? false,
          _players.previous,
        ),
        _button(
          now?.playing ?? false ? _Transport.pause : _Transport.play,
          1,
          now != null,
          _players.playPause,
        ),
        _button(_Transport.next, 2, now?.canNext ?? false, _players.next),
        Positioned.fromRect(
          rect: nowPlayingProgress,
          child: CustomPaint(painter: _ProgressPainter(fraction)),
        ),
        if (length > Duration.zero)
          Positioned(
            left: nowPlayingProgress.left,
            width: nowPlayingProgress.width,
            top: nowPlayingProgress.bottom + 1,
            child: Row(
              children: [
                Text(mediaTime(position), style: timeStyle),
                const Spacer(),
                Text('−${mediaTime(length - position)}', style: timeStyle),
              ],
            ),
          ),
      ],
    );
  }
}

class _ProgressPainter extends CustomPainter {
  const _ProgressPainter(this.fraction);
  final double fraction;

  @override
  void paint(Canvas canvas, Size size) {
    final track = RRect.fromRectAndRadius(
      Rect.fromCenter(
        center: size.center(Offset.zero),
        width: size.width,
        height: 4,
      ),
      const Radius.circular(2),
    );
    canvas
      ..drawRRect(track, Paint()..color = nowPlayingTrack)
      ..save()
      ..clipRect(Rect.fromLTWH(0, 0, size.width * fraction, size.height))
      ..drawRRect(track, Paint()..color = nowPlayingElapsed)
      ..restore();
  }

  @override
  bool shouldRepaint(_ProgressPainter old) => old.fraction != fraction;
}

enum _Transport { previous, play, pause, next }

/// The transport glyphs: two side by side triangles (⏮ to the left, ⏭ to the right), a
/// triangle (▶) or two bars (pause).
class _TransportPainter extends CustomPainter {
  const _TransportPainter(this.kind, this.color);
  final _Transport kind;
  final Color color;

  @override
  void paint(Canvas canvas, Size size) {
    final paint = Paint()..color = color;
    final w = size.width;
    final h = size.height;
    switch (kind) {
      case _Transport.play:
        canvas.drawPath(
          Path()
            ..moveTo(0, 0)
            ..lineTo(w, h / 2)
            ..lineTo(0, h)
            ..close(),
          paint,
        );
      case _Transport.pause:
        canvas
          ..drawRRect(
            RRect.fromLTRBR(1, 0, w * 0.38, h, const Radius.circular(1.5)),
            paint,
          )
          ..drawRRect(
            RRect.fromLTRBR(w * 0.62, 0, w - 1, h, const Radius.circular(1.5)),
            paint,
          );
      case _Transport.previous || _Transport.next:
        final half = w / 2;
        for (final start in [0.0, half]) {
          final path = kind == _Transport.next
              ? (Path()
                  ..moveTo(start, 0)
                  ..lineTo(start + half, h / 2)
                  ..lineTo(start, h))
              : (Path()
                  ..moveTo(start + half, 0)
                  ..lineTo(start, h / 2)
                  ..lineTo(start + half, h));
          canvas.drawPath(path..close(), paint);
        }
    }
  }

  @override
  bool shouldRepaint(_TransportPainter old) =>
      old.kind != kind || old.color != color;
}

class _SliderModule extends StatelessWidget {
  const _SliderModule({
    this.onOpen,
    this.onOutputs,
    required this.title,
    required this.value,
    required this.enabled,
    required this.low,
    required this.high,
    required this.onChanged,
  });

  final String title;
  final double value;
  final bool enabled;
  final GlyphKind low;
  final GlyphKind high;
  final ValueChanged<double> onChanged;

  /// The title opens the detail.
  final VoidCallback? onOpen;

  /// The audio outputs circle (Sound only): opens the detail with the outputs.
  final VoidCallback? onOutputs;

  /// With the outputs circle the track is shorter.
  double get _length =>
      onOutputs == null ? trackLength : trackLength - outputsShift;

  void _set(Offset local) {
    if (enabled) onChanged(sliderValue(local.dx - trackLeft, length: _length));
  }

  @override
  Widget build(BuildContext context) => GestureDetector(
    behavior: HitTestBehavior.opaque,
    onTapDown: (d) {
      // Above the track (the title): the detail; on the track: the value.
      if (onOpen != null && d.localPosition.dy < trackCenter - 10) {
        onOpen!();
      } else if (onOutputs != null &&
          (d.localPosition.dx - outputsCenter).abs() <= outputsSide / 2 + 4) {
        onOutputs!();
      } else {
        _set(d.localPosition);
      }
    },
    onHorizontalDragUpdate: (d) => _set(d.localPosition),
    child: Stack(
      children: [
        Positioned(
          left: 16,
          top: 10,
          child: Text(title, style: _text(titleSize, weight: FontWeight.w600)),
        ),
        Positioned(
          left: 15 - 8,
          top: trackCenter - 8,
          width: 16 + 8,
          height: 16,
          child: Glyph(low, color: textColor, size: 16),
        ),
        Positioned(
          left: trackLeft,
          top: trackCenter - trackHeight / 2,
          width: _length,
          height: trackHeight,
          child: CustomPaint(
            painter: _Track(enabled ? value : null, length: _length),
          ),
        ),
        if (onOutputs != null)
          Positioned(
            key: const ValueKey('audio outputs'),
            left: outputsCenter - outputsSide / 2,
            top: trackCenter - outputsSide / 2,
            width: outputsSide,
            height: outputsSide,
            child: const DecoratedBox(
              decoration: BoxDecoration(
                color: outputsFill,
                shape: BoxShape.circle,
              ),
              child: CustomPaint(painter: _OutputsGlyph()),
            ),
          ),
        Positioned(
          left: 260 - (onOutputs == null ? 0 : outputsShift),
          top: trackCenter - 8.5,
          width: 17,
          height: 17,
          child: Glyph(high, color: textColor, size: 17),
        ),
      ],
    ),
  );
}

class _Track extends CustomPainter {
  const _Track(this.value, {this.length = trackLength});

  /// Null: slider not adjustable (only the track, as in the reference).
  final double? value;
  final double length;

  @override
  void paint(Canvas canvas, Size size) {
    final radius = Radius.circular(size.height / 2);
    canvas.drawRRect(
      RRect.fromRectAndRadius(Offset.zero & size, radius),
      Paint()..color = trackRest,
    );
    final v = value;
    if (v == null) return;
    canvas.drawRRect(
      RRect.fromRectAndRadius(
        Rect.fromLTWH(0, 0, sliderFill(v, length: length), size.height),
        radius,
      ),
      Paint()..color = const Color(0xFFFFFFFF),
    );
  }

  @override
  bool shouldRepaint(_Track old) => old.value != value || old.length != length;
}

/// The glyph of the audio outputs circle: a pair of headphones, white, ~13 pt.
class _OutputsGlyph extends CustomPainter {
  const _OutputsGlyph();

  @override
  void paint(Canvas canvas, Size size) {
    final c = size.center(Offset.zero);
    final paint = Paint()
      ..color = const Color(0xFFFFFFFF)
      ..style = PaintingStyle.stroke
      ..strokeWidth = 1.6
      ..strokeCap = StrokeCap.round;
    canvas.drawArc(
      Rect.fromCircle(center: c + const Offset(0, 0.5), radius: 5.5),
      3.3,
      2.82,
      false,
      paint,
    );
    final fill = Paint()..color = const Color(0xFFFFFFFF);
    for (final dx in [-5.5, 5.5]) {
      canvas.drawRRect(
        RRect.fromRectAndRadius(
          Rect.fromCenter(center: c + Offset(dx, 3.5), width: 3, height: 6),
          const Radius.circular(1.2),
        ),
        fill,
      );
    }
  }

  @override
  bool shouldRepaint(_OutputsGlyph old) => false;
}

class _StackShadow extends CustomPainter {
  const _StackShadow({
    required this.origin,
    required this.height,
    required this.slots,
  });

  final Offset origin;
  final double height;
  final List<ModuleSlot> slots;

  @override
  void paint(Canvas canvas, Size size) {
    var modules = Path();
    for (final slot in slots) {
      modules = Path.combine(
        PathOperation.union,
        modules,
        Path()..addRRect(
          // Like the glass: 1 pt more on the left and right.
          RRect.fromRectAndRadius(
            Rect.fromLTRB(
              slot.rect.left - 1,
              slot.rect.top,
              slot.rect.right + 1,
              slot.rect.bottom,
            ).shift(origin),
            Radius.circular(slot.radius),
          ),
        ),
      );
    }
    final area = Rect.fromLTRB(origin.dx, 0, size.width, origin.dy + height);
    canvas
      ..save()
      ..clipPath(
        Path.combine(
          PathOperation.difference,
          Path()..addRect(Offset.zero & size),
          modules,
        ),
      )
      ..drawRect(
        area,
        Paint()
          ..color = shadowColor
          ..maskFilter = const MaskFilter.blur(BlurStyle.normal, shadowSigma),
      )
      ..restore();
  }

  // Decoration only: without this, a full-screen CustomPaint counts as hit and swallows the
  // clicks outside the modules that should close the Control Center.
  @override
  bool? hitTest(Offset position) => false;

  @override
  bool shouldRepaint(_StackShadow old) =>
      old.origin != origin ||
      old.height != height ||
      !listEquals(old.slots, slots);
}
