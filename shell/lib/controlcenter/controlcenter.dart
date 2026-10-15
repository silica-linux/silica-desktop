// The Control Center, from plan/reference/controlcenter.md. In the dark appearance over grey
// it is pixel-identical to light (white texts, same fills): only the glass changes, and
// Ossido draws it.
//
// A stack of separate `regular` glass modules, anchored at the top right: two 140 pt
// columns 12 pt apart, 14 pt from the right edge, the first module at y 42; 64 pt rows at
// a 76 pitch. 140 × 64 capsules (radius 32) with a 36 pt disc 14 pt from the edge and the
// text at 56 pt; the media module 140 × 140 (radius 35); Display and Sound 292 × 64
// (radius ~28) with a slider whose knob shows only on hover. A wide soft shadow around the stack; opening and
// closing with the opacities and the scale measured frame by frame.
//
// Which modules, in which order and size: `modules.dart` (the reference's default, or the
// user's arrangement from Edit Controls). Modules with a real effect: Wi-Fi, Bluetooth,
// Focus, Dark Mode, Screen Capture, Display, Sound (from silica-daemon and Ossido), Now
// playing (MPRIS), the battery if there is one. Nearby sharing, window staging and screen
// mirroring are in the reference's grid but Silica has no backend for them yet: they show
// disabled, saying so (#140 and the stage view and display issues). Click on the icon disc:
// on or off; click on the text (or on the slider title): the detail panel (`detail.dart`),
// where there is one; with the pointer over a module with a detail the ">" arrow appears.
// Esc from the detail goes back to the Center.
//
// The component keeps running on a full-screen overlay layer: when closed it draws nothing
// and takes neither pointer nor keyboard (silica/layer).

import 'dart:async';
import 'dart:io' show File;

import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';
import 'package:silica_ui/silica_ui.dart';

import '../l10n/l10n.dart';
import '../menubar/menubar.dart' show barHeight;
import '../system/layer_control.dart';
import '../system/media.dart';
import '../system/shadow_image.dart';
import '../system/shell_channel.dart';
import '../system/desktop_entries.dart';
import '../system/system_status.dart';
import 'detail.dart';
import 'glyphs.dart';
import 'edit.dart';
import 'hud.dart';
import 'modules.dart';

const double stackWidth = 292;
const double columnWidth = 140;
const double gap = 12;
const double rightMargin = 14;
const double stackTop = 42;

/// The privacy pill: 24 pt tall at y 40, 6 pt above the first module, which moves down
/// 28 pt (Wi-Fi at y 70).
const double pillTop = 40;
const double pillHeight = 24;
const double pillShift = 28;
const Color pillIcon = Color(0xFF6E7CFF);
const Color pillText = Color(0xFFF0F0F1);
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

/// The layout of the present modules, from the top (coordinates relative to the stack):
/// [modules] packed into the grid (modules.dart), each with its shape's radius.
Map<String, ModuleSlot> layoutModules(
  SystemSnapshot s, [
  List<CcModule> modules = defaultModules,
]) {
  final present = presentModules(modules, s);
  final frames = packModules(present);
  return {
    for (final m in present)
      if (frames[m.id] case final rect?)
        m.id: ModuleSlot(rect, switch (m.size) {
          ModuleSize.small => rowHeight / 2,
          ModuleSize.medium => capsuleRadius,
          ModuleSize.large => mediaRadius,
          ModuleSize.wide => sliderRadius,
        }),
  };
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
    this.env,
    this.settingsApp,
  });

  final SystemSource system;
  final bool startOpen;

  /// The environment for the arrangement file (tests); null: the process environment.
  final Map<String, String>? env;

  /// The Settings app (tests); null: looked up among the installed apps.
  final DesktopEntry? settingsApp;

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

  /// The arrangement: the user's, or the reference's default.
  late List<CcModule> _modules = loadModules(env: widget.env);

  @override
  void initState() {
    super.initState();
    _anim = AnimationController(vsync: this)
      ..addListener(() => setState(() {}));
    widget.system.addListener(_changed);
    _commands = ShellChannel.instance.commands.listen((id) {
      if (id == 'controlcenter.toggle') _toggle();
    });
    _dismissed = ShellChannel.instance.transientDismissed.listen(
      (_) => _transientDismissed(),
    );
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
    _dismissed?.cancel();
    _anim.dispose();
    _dndTimer?.cancel();
    _detailAnim.dispose();
    _editAnim.dispose();
    _focus.dispose();
    _menuFocus.dispose();
    super.dispose();
  }

  void _changed() => setState(() {});

  // A press on the bar's icon while the Center is open: Ossido first ends the panel
  // (transientDismissed, the press goes through), then the bar asks for the toggle, which
  // must not open it again.
  void _toggle() {
    if (!_open && _dismissedAt != null) {
      final since = DateTime.now().difference(_dismissedAt!);
      if (since < const Duration(milliseconds: 400)) return;
    }
    _open ? _hide() : _show();
  }

  StreamSubscription<void>? _dismissed;
  DateTime? _dismissedAt;

  /// A press outside the open Center or status menu ended it (Ossido, #149).
  void _transientDismissed() {
    if (_wifiMenuRight != null) _closeWifiMenu();
    if (_open) {
      _dismissedAt = DateTime.now();
      _hide();
    }
  }

  /// The visible panel or status menu: the input region and the transient follow it, so
  /// the windows below keep the pointer (#149).
  Rect? _region;

  void _setRegion(Rect rect, TransientPolicy policy) {
    if (rect == _region) return;
    _region = rect;
    WidgetsBinding.instance.addPostFrameCallback((_) async {
      if (!mounted || (!_open && _wifiMenuRight == null)) return;
      final supported = await LayerControl.transient(
        rects: [rect],
        policy: policy,
      );
      if (!mounted || (!_open && _wifiMenuRight == null)) return;
      LayerControl.inputRegion(supported ? [rect] : null);
    });
  }

  /// The status menu opened from the bar (Wi-Fi, Focus): the right edge of the icon.
  double? _wifiMenuRight;
  String? _statusMenuKey;
  final FocusNode _menuFocus = FocusNode();

  /// The panels the bar's status items open.
  static const Set<String> statusMenus = {
    'wifi',
    'dnd',
    'media',
    'bluetooth',
    'sound',
  };

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
    _region = null;
    LayerControl.inputRegion(const []);
    LayerControl.keyboard('exclusive');
    WidgetsBinding.instance.addPostFrameCallback(
      (_) => _menuFocus.requestFocus(),
    );
    final system = widget.system;
    final data = switch (key) {
      'wifi' => DetailData(networks: await system.wifiNetworks()),
      'sound' => DetailData(outputs: await system.audioOutputs()),
      'bluetooth' => DetailData(devices: await system.bluetoothDevices()),
      _ => null,
    };
    if (data != null &&
        mounted &&
        _wifiMenuRight != null &&
        _statusMenuKey == key) {
      setState(() => _detailData = data);
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
      _region = null;
      LayerControl.clearTransient();
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
    // A status menu is a system panel (INPUT-01 (f)): clicks, drags and the wheel outside
    // pass; the keys stay here.
    _setRegion(rect.inflate(2), TransientPolicy.panel);
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
    _region = null;
    LayerControl.inputRegion(const []);
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
    final slot = layoutModules(widget.system.value, _modules)[key];
    final scale = detailTimeScale(slot == null ? 0 : slot.rect.top - detailTop);
    if (_detailClosing) {
      return 1 - detailProgress(_detailAnim.value / detailBackStretch, scale);
    }
    return detailProgress(_detailAnim.value, scale);
  }

  void _hide() {
    _detail = null;
    _detailClosing = false;
    _editing = false;
    _moduleMenu = null;
    setState(() => _open = false);
    ShellChannel.instance.broadcast('controlcenter.open', '0');
    _region = null;
    LayerControl.clearTransient();
    LayerControl.keyboard('none');
    LayerControl.inputRegion(const []);
    _anim.duration = closeDuration;
    _anim.forward(from: 0);
  }

  @override
  Widget build(BuildContext context) {
    // Everything in one overlay entry: dragging controls in edit mode needs an Overlay
    // (the shell's components have none), and the entry follows every rebuild.
    _layer.markNeedsBuild();
    return Overlay(initialEntries: [_layer]);
  }

  late final OverlayEntry _layer = OverlayEntry(
    builder: (context) => Stack(
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
    ),
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
    final slots = layoutModules(snapshot, _modules);
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
        final top = stackTop + (snapshot.privacy.isEmpty ? 0 : pillShift);
        final origin = Offset(left, top);
        final shadow = _StackShadow(
          origin: origin,
          height: totalHeight,
          slots: shadowSlots,
        );
        if (_open) {
          _setRegion(
            Rect.fromLTWH(
              left - detailPad.dx,
              top - detailPad.dy,
              detailWidth,
              totalHeight + detailPad.dy,
            ),
            TransientPolicy.panel,
          );
        }
        return Focus(
          focusNode: _focus,
          onKeyEvent: (node, event) {
            if (event is KeyDownEvent &&
                event.logicalKey == LogicalKeyboardKey.escape) {
              // From the detail back to the Center; from the Center (or edit mode, gallery
              // included) it closes.
              _detail != null && !_detailClosing && !_editing
                  ? _closeDetail()
                  : _hide();
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
                  // Edit mode dims the screen behind (~12 levels).
                  child: _editing
                      ? Opacity(
                          opacity: opacity,
                          child: const ColoredBox(color: editDim),
                        )
                      : null,
                ),
              ),
              // The shadow is an image (CachedShadow): drawn once per layout, it fades with the
              // panel under Impeller, which cannot fade a blurred draw under a clip.
              Positioned.fill(
                child: RepaintBoundary(
                  child: CachedShadow(
                    cacheKey: shadow.cacheKey,
                    paintShadow: shadow.paintShadow,
                    opacity: opacity,
                    hittable: false,
                  ),
                ),
              ),
              if (snapshot.privacy.firstOrNull case final use?)
                Positioned(
                  left: left,
                  width: stackWidth,
                  top: pillTop,
                  height: pillHeight,
                  child: Opacity(
                    opacity: opacity * othersAlpha,
                    // As tall as the band, as wide as its content, centred.
                    child: Center(
                      child: SizedBox(
                        height: pillHeight,
                        child: _PrivacyPill(
                          use: use,
                          onTap: _settingsAt('privacy'),
                        ),
                      ),
                    ),
                  ),
                ),
              Positioned(
                left: left - detailPad.dx,
                top: top - detailPad.dy,
                width: detailWidth,
                height: totalHeight + detailPad.dy + _belowStack,
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
                                child: _arranged(key, value, snapshot),
                              ),
                            ),
                          ),
                        // Edit Controls: centred 16 pt under the last module; in edit mode the
                        // remove badges and the eight empty slots instead.
                        if (_detail == null && !_editing)
                          Positioned(
                            left:
                                (stackWidth - editButton.width) / 2 +
                                detailPad.dx,
                            top: height + editButtonGap + detailPad.dy,
                            width: editButton.width,
                            height: editButton.height,
                            child: _EditButton(
                              label: l10nOf(context).ccEditControls,
                              onTap: _startEditing,
                            ),
                          ),
                        if (_editing) ...[
                          for (final MapEntry(:key, :value) in slots.entries)
                            Positioned(
                              left:
                                  value.rect.left - badgeOutside + detailPad.dx,
                              top: value.rect.top - badgeOutside + detailPad.dy,
                              width: badgeSide,
                              height: badgeSide,
                              child: RemoveBadge(
                                label:
                                    '${l10nOf(context).ccRemove} ${_label(key)}',
                                onTap: () => _removeModule(key),
                              ),
                            ),
                          for (var i = 0; i < emptySlots; i++)
                            Positioned(
                              left: (i % 4) * pitch + detailPad.dx,
                              top:
                                  height +
                                  gap +
                                  (i ~/ 4) * pitch +
                                  detailPad.dy,
                              width: cell,
                              height: cell,
                              child: Opacity(
                                opacity: slotsOpacity(
                                  _editAnim.value * editTotalMs,
                                ),
                                child: EmptySlot(onDrop: _placeModule),
                              ),
                            ),
                        ],
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
              if (_editing) _gallery(constraints.biggest, opacity),
              if (_moduleMenu case (final key, final at))
                Positioned.fill(
                  child: MenuHost(
                    key: ValueKey('module menu $key $at'),
                    entries: _moduleMenuEntries(key),
                    anchor:
                        at - Offset(menuSize(_moduleMenuEntries(key)).width, 0),
                    top: barHeight,
                    onDismissed: () => setState(() => _moduleMenu = null),
                  ),
                ),
            ],
          ),
        );
      },
    );
  }

  /// Room under the stack: the Edit Controls button, or the empty slots in edit mode.
  double get _belowStack =>
      _editing ? gap + cell + pitch : editButtonGap + editButton.height;

  /// The gallery, centred on the screen (shorter screens: as tall as fits).
  Widget _gallery(Size screen, double opacity) {
    final height = gallerySize.height.clamp(0.0, screen.height - 80);
    final present = {for (final m in _modules) m.id};
    final available = {
      for (final id in moduleSizes.keys)
        if (moduleAvailable(id, widget.system.value)) id,
    };
    return Positioned(
      left: (screen.width - gallerySize.width) / 2,
      top: (screen.height - height) / 2,
      width: gallerySize.width,
      height: height,
      child: Opacity(
        opacity: opacity * galleryOpacity(_editAnim.value * editTotalMs),
        child: DecoratedBox(
          decoration: BoxDecoration(
            borderRadius: BorderRadius.circular(galleryRadius),
            boxShadow: const [
              BoxShadow(
                color: Color(0x40000000),
                blurRadius: 40,
                offset: Offset(0, 16),
              ),
            ],
          ),
          child: ControlGallery(
            present: present,
            available: available,
            preview: _preview,
            label: _label,
            onAdd: _placeModule,
            onDone: _stopEditing,
          ),
        ),
      ),
    );
  }

  /// A module in the stack: the secondary click opens its menu; in edit mode it can be
  /// dragged to another place, and controls dropped on it go before it.
  Widget _arranged(String key, ModuleSlot slot, SystemSnapshot s) {
    final module = GestureDetector(
      onSecondaryTapUp: (d) =>
          setState(() => _moduleMenu = (key, d.globalPosition)),
      child: _module(key, s),
    );
    if (!_editing) return _ModuleHover(radius: slot.radius, child: module);
    return DragTarget<String>(
      onAcceptWithDetails: (d) => _placeModule(d.data, before: key),
      builder: (context, candidates, _) => Draggable<String>(
        data: key,
        feedback: Opacity(
          opacity: 0.85,
          child: SizedBox.fromSize(
            size: slot.rect.size,
            child: DecoratedBox(
              decoration: BoxDecoration(
                color: const Color(0xFF6A6B70),
                borderRadius: BorderRadius.circular(slot.radius),
              ),
              child: IgnorePointer(child: _module(key, s)),
            ),
          ),
        ),
        childWhenDragging: const SizedBox.expand(),
        // In edit mode the modules don't act: they are dragged.
        child: Opacity(
          opacity: candidates.isEmpty ? 1 : 0.6,
          child: IgnorePointer(child: module),
        ),
      ),
    );
  }

  /// The glass of a module: 1 pt more on the left and right.
  Rect _glassRect(Rect r) =>
      Rect.fromLTRB(r.left - 1, r.top, r.right + 1, r.bottom);

  /// The Settings app, if installed (its first argument names the pane).
  late final DesktopEntry? _settingsApp =
      widget.settingsApp ??
      loadDesktopEntries(locale: systemLanguage()).values
          .where((e) => e.id.toLowerCase().endsWith('.settings'))
          .firstOrNull;

  /// Opens Settings at [pane] and closes the Center or the status menu; null without the
  /// app (the footer item is not shown then).
  VoidCallback? _settingsAt(String pane) {
    final app = _settingsApp;
    if (app == null) return null;
    return () {
      _closeWifiMenu();
      if (_open) _hide();
      ShellChannel.instance.launch('${app.exec} --dart-arg $pane');
    };
  }

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
        onSettings: _settingsAt('wifi'),
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
        onSettings: _settingsAt('sound'),
      ),
      'display' => displayDetail(
        l: l,
        s: s,
        onBrightness: system.setBrightness,
        track: track,
        dark: MediaQuery.maybePlatformBrightnessOf(context) == Brightness.dark,
        onDark: () => ShellChannel.instance.execute('system.dark_toggle'),
        onSettings: _settingsAt('displays'),
      ),
      'bluetooth' => bluetoothDetail(
        l: l,
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
        onSettings: _settingsAt('bluetooth'),
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
        onSettings: _settingsAt('focus'),
      ),
      _ => null,
    };
  }

  /// While building a gallery preview: the size to draw it at.
  ModuleSize? _sizeOverride;

  /// Module [key] at [size], for the gallery.
  Widget _preview(String key, ModuleSize size) {
    _sizeOverride = size;
    try {
      return _module(key, widget.system.value);
    } finally {
      _sizeOverride = null;
    }
  }

  /// The control's name, for the gallery and the badges.
  String _label(String key) {
    final l = l10nOf(context);
    return switch (key) {
      'wifi' => 'Wi-Fi',
      'bluetooth' => 'Bluetooth',
      'nearby' => l.ccNearbySharing,
      'media' => l.ccNowPlaying,
      'dnd' => l.ccFocus,
      'dark' => l.ccDarkMode,
      'capture' => l.ccScreenCapture,
      'staging' => l.ccStageView,
      'mirroring' => l.ccScreenMirroring,
      'battery' => l.ccBattery,
      'keyboard' => l.ccKeyboardBrightness,
      'display' => l.ccDisplay,
      _ => l.ccSound,
    };
  }

  // Edit Controls: the arrangement changes and is saved at once.

  void _setModules(List<CcModule> modules) {
    setState(() => _modules = modules);
    saveModules(modules, env: widget.env);
  }

  void _removeModule(String key) => _setModules([
    for (final m in _modules)
      if (m.id != key) m,
  ]);

  /// Adds [key] (from the gallery) or moves it (in the Center) before [before], or at the
  /// end.
  void _placeModule(String key, {String? before}) {
    if (key == before) return;
    final existing = _modules.where((m) => m.id == key).firstOrNull;
    final module =
        existing ?? CcModule(key, moduleSizes[key]?.first ?? ModuleSize.small);
    final rest = [
      for (final m in _modules)
        if (m.id != key) m,
    ];
    final at = before == null
        ? rest.length
        : rest.indexWhere((m) => m.id == before);
    rest.insert(at < 0 ? rest.length : at, module);
    _setModules(rest);
  }

  void _resizeModule(String key, ModuleSize size) => _setModules([
    for (final m in _modules) m.id == key ? m.withSize(size) : m,
  ]);

  bool _editing = false;
  late final AnimationController _editAnim = AnimationController(
    vsync: this,
    duration: Duration(milliseconds: editTotalMs.round()),
  )..addListener(() => setState(() {}));

  void _startEditing() {
    _closeDetail();
    setState(() => _editing = true);
    _editAnim.forward(from: 0);
  }

  void _stopEditing() {
    if (!_editing) return;
    setState(() => _editing = false);
    _editAnim.value = 0;
  }

  /// The module menu (secondary click): its top-right corner on the pointer.
  (String, Offset)? _moduleMenu;

  List<MenuEntry> _moduleMenuEntries(String key) {
    final l = l10nOf(context);
    final sizes = moduleSizes[key] ?? const <ModuleSize>[];
    final current = _sizeOf(key);
    MenuAction size(String label, ModuleSize s) => MenuAction(
      label,
      checked: current == s,
      enabled: sizes.contains(s) && sizes.length > 1,
      onSelected: () => _resizeModule(key, s),
    );
    return [
      MenuSection(l.ccSize),
      size(l.ccSizeSmall, ModuleSize.small),
      size(l.ccSizeMedium, ModuleSize.medium),
      size(l.ccSizeLarge, ModuleSize.large),
      const MenuDivider(),
      // The menu bar can't host Control Center controls yet (#59).
      MenuAction(l.ccCopyToMenuBar, enabled: false),
      const MenuDivider(),
      MenuAction(l.ccRemove, onSelected: () => _removeModule(key)),
    ];
  }

  /// The size of module [key] in the arrangement.
  ModuleSize _sizeOf(String key) =>
      _modules.where((m) => m.id == key).firstOrNull?.size ?? ModuleSize.small;

  /// A toggle without a detail: a 64 pt round tile when small, a 140 × 64 capsule (disc and
  /// label) when medium. Without [onTap] it is disabled: Silica can't do it yet.
  Widget _toggleModule(
    String key, {
    required GlyphKind glyph,
    required String label,
    required bool active,
    required VoidCallback? onTap,
  }) {
    final l = l10nOf(context);
    if ((_sizeOverride ?? _sizeOf(key)) == ModuleSize.small) {
      return _RoundToggle(
        glyph: glyph,
        label: label,
        active: active,
        onTap: onTap,
      );
    }
    return _Capsule(
      glyph: glyph,
      title: label,
      subtitle: onTap == null
          ? l.ccNotAvailableYet
          : (active ? l.ccOn : l.ccOff),
      active: active,
      enabled: onTap != null,
      onTap: onTap,
    );
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
      'dark' => _toggleModule(
        key,
        glyph: GlyphKind.contrast,
        label: l.ccDarkMode,
        active:
            MediaQuery.maybePlatformBrightnessOf(context) == Brightness.dark,
        onTap: () => ShellChannel.instance.execute('system.dark_toggle'),
      ),
      'capture' => _toggleModule(
        key,
        glyph: GlyphKind.screenCapture,
        label: l.ccScreenCapture,
        active: false,
        onTap: () {
          _hide();
          ShellChannel.instance.execute('screenshot.toolbar');
        },
      ),
      // In the reference's grid, with no backend in Silica yet: shown disabled, saying so
      // (nearby sharing #140; window staging and screen mirroring need Ossido's stage view
      // and a second display).
      'nearby' => _Capsule(
        glyph: GlyphKind.nearby,
        title: l.ccNearbySharing,
        subtitle: l.ccNotAvailableYet,
        active: false,
        enabled: false,
      ),
      'staging' => _toggleModule(
        key,
        glyph: GlyphKind.staging,
        label: l.ccStageView,
        active: false,
        onTap: null,
      ),
      'mirroring' => _toggleModule(
        key,
        glyph: GlyphKind.mirroring,
        label: l.ccScreenMirroring,
        active: false,
        onTap: null,
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
      // The keyboard backlight (UPower), offered only with a backlit keyboard.
      'keyboard' => _SliderModule(
        title: l.ccKeyboardBrightness,
        value: s.keyboardBrightness,
        enabled: s.keyboardAvailable,
        low: GlyphKind.keyboard,
        high: GlyphKind.keyboard,
        onChanged: (v) => ShellChannel.instance.execute(
          'system.keyboard_brightness',
          {'value': v},
        ),
      ),
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

class _Capsule extends StatelessWidget {
  const _Capsule({
    required this.glyph,
    required this.title,
    required this.subtitle,
    required this.active,
    this.onTap,
    this.onOpen,
    this.glyphLevel = 1,
    this.activeGlyph = glyphOn,
    this.enabled = true,
  });

  final GlyphKind glyph;

  /// False when Silica can't do it yet: the module is dimmed and does not respond.
  final bool enabled;

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
  Widget build(BuildContext context) {
    final widget = this;
    final hover = ModuleHoverScope.of(context);
    final arrowable = widget.onOpen != null && widget.enabled;
    final capsule = Stack(
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
          right:
              10 + (arrowable ? hover.progress * (hoverChevronRight + 2) : 0),
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
        if (arrowable)
          Positioned(
            right: hoverChevronRight,
            top: (rowHeight - 12) / 2,
            child: IgnorePointer(
              child: Opacity(
                key: const ValueKey('detail chevron'),
                opacity: hover.progress,
                child: const Chevron(color: textColor),
              ),
            ),
          ),
      ],
    );
    if (widget.enabled) return capsule;
    return Semantics(
      enabled: false,
      child: AbsorbPointer(
        child: Opacity(opacity: disabledOpacity, child: capsule),
      ),
    );
  }
}

/// A module Silica can't drive yet: dimmed (provisional, the reference shows none).
const double disabledOpacity = 0.45;

/// "Until this evening": 19:00 today, or the end of the day once past it (provisional).
Duration untilEvening(DateTime now) {
  final evening = DateTime(now.year, now.month, now.day, 19);
  final end = now.isBefore(evening)
      ? evening
      : DateTime(now.year, now.month, now.day + 1);
  return end.difference(now);
}

/// The ">" arrow of modules with a detail, at the right end of the title row: it fades in
/// with the module's hover and the text shortens to make room (CC-HOVER).
const double hoverChevronRight = 16;

/// Hover of a Control Center module (controlcenter.md, CC-HOVER): the whole module lifts by
/// about 7 levels (white at 4.7 %), with no scale; inner buttons have no hover of their own.
/// The change starts after ~50 ms (10 %), is half done at ~150 ms and done at ~300 ms, the
/// same in and out and for every module.
const Duration hoverDuration = Duration(milliseconds: 300);
const Color moduleHoverLift = Color(0x0CFFFFFF);

/// The volume knob (~18 x 14 pt) shows ~330 ms after the pointer enters the module and goes
/// ~60 ms after it leaves.
const Duration knobDelay = Duration(milliseconds: 330);
const Duration knobLeaveDelay = Duration(milliseconds: 60);
const Size knobSize = Size(18, 14);

/// 10 % at 50 ms, 50 % at 150 ms, 100 % at 300 ms: a mild ease-out.
class HoverCurve extends Curve {
  const HoverCurve();

  static const List<(double, double)> _points = [
    (0, 0),
    (50 / 300, 0.1),
    (150 / 300, 0.5),
    (1, 1),
  ];

  @override
  double transformInternal(double t) {
    for (var i = 1; i < _points.length; i++) {
      final (x1, y1) = _points[i];
      if (t <= x1) {
        final (x0, y0) = _points[i - 1];
        return y0 + (y1 - y0) * (t - x0) / (x1 - x0);
      }
    }
    return 1;
  }
}

/// What a module's content reads from its wrapper: whether the pointer is on the module and
/// how far the hover has gone (0..1).
@immutable
class ModuleHoverState {
  const ModuleHoverState(this.hovered, this.progress);
  final bool hovered;
  final double progress;
}

class ModuleHoverScope extends InheritedWidget {
  const ModuleHoverScope({
    super.key,
    required this.state,
    required super.child,
  });

  final ModuleHoverState state;

  static ModuleHoverState of(BuildContext context) =>
      context.dependOnInheritedWidgetOfExactType<ModuleHoverScope>()?.state ??
      const ModuleHoverState(false, 0);

  @override
  bool updateShouldNotify(ModuleHoverScope old) =>
      old.state.hovered != state.hovered ||
      old.state.progress != state.progress;
}

/// Wraps a module of the stack: the hover lift over the whole module (its [radius]) and the
/// scope its content reads.
class _ModuleHover extends StatefulWidget {
  const _ModuleHover({required this.radius, required this.child});

  final double radius;
  final Widget child;

  @override
  State<_ModuleHover> createState() => _ModuleHoverState();
}

class _ModuleHoverState extends State<_ModuleHover>
    with SingleTickerProviderStateMixin {
  late final AnimationController _controller = AnimationController(
    vsync: this,
    duration: hoverDuration,
  );
  // Entering follows the curve; leaving is its mirror in time (10 % at 55 ms, half at 150 ms).
  double get _progress {
    const curve = HoverCurve();
    final v = _controller.value;
    return _hover ? curve.transform(v) : curve.flipped.transform(v);
  }

  bool _hover = false;

  @override
  void dispose() {
    _controller.dispose();
    super.dispose();
  }

  void _set(bool hover) {
    _hover = hover;
    hover ? _controller.forward() : _controller.reverse();
    setState(() {});
  }

  @override
  Widget build(BuildContext context) {
    return MouseRegion(
      onEnter: (_) => _set(true),
      onExit: (_) => _set(false),
      child: AnimatedBuilder(
        animation: _controller,
        child: widget.child,
        builder: (context, child) => ModuleHoverScope(
          state: ModuleHoverState(_hover, _progress),
          child: Stack(
            fit: StackFit.passthrough,
            clipBehavior: Clip.none,
            children: [
              // Same extent as the module's glass (1 pt more left and right).
              Positioned(
                left: -1,
                right: -1,
                top: 0,
                bottom: 0,
                child: IgnorePointer(
                  child: DecoratedBox(
                    key: const ValueKey('module hover'),
                    decoration: BoxDecoration(
                      color: moduleHoverLift.withValues(
                        alpha: moduleHoverLift.a * _progress,
                      ),
                      borderRadius: BorderRadius.circular(widget.radius),
                    ),
                  ),
                ),
              ),
              child!,
            ],
          ),
        ),
      ),
    );
  }
}

/// The volume knob: a light capsule, shown only while the pointer is on the module (after
/// [knobDelay], gone [knobLeaveDelay] after leaving).
class _Knob extends StatefulWidget {
  const _Knob();

  @override
  State<_Knob> createState() => _KnobState();
}

class _KnobState extends State<_Knob> {
  bool _shown = false;
  bool? _target;
  Timer? _timer;

  @override
  void didChangeDependencies() {
    super.didChangeDependencies();
    final hovered = ModuleHoverScope.of(context).hovered;
    if (hovered == _target) return;
    _target = hovered;
    _timer?.cancel();
    if (hovered == _shown) return;
    _timer = Timer(hovered ? knobDelay : knobLeaveDelay, () {
      if (mounted) setState(() => _shown = hovered);
    });
  }

  @override
  void dispose() {
    _timer?.cancel();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    if (!_shown) return const SizedBox.shrink();
    return const DecoratedBox(
      key: ValueKey('volume knob'),
      decoration: BoxDecoration(
        color: Color(0xFFF0F0F1),
        borderRadius: BorderRadius.all(Radius.circular(7)),
        boxShadow: [
          BoxShadow(
            color: Color(0x40000000),
            blurRadius: 3,
            offset: Offset(0, 1),
          ),
        ],
      ),
    );
  }
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

  /// Null: Silica can't do it yet, the tile is dimmed and does not respond.
  final VoidCallback? onTap;

  @override
  State<_RoundToggle> createState() => _RoundToggleState();
}

class _RoundToggleState extends State<_RoundToggle> {
  @override
  Widget build(BuildContext context) {
    final enabled = widget.onTap != null;
    final tile = DecoratedBox(
      decoration: BoxDecoration(
        shape: BoxShape.circle,
        color: widget.active ? discOn : null,
      ),
      child: Center(
        child: Glyph(
          widget.glyph,
          color: widget.active ? const Color(0xFF000000) : glyphOff,
          size: 24,
        ),
      ),
    );
    return Semantics(
      button: true,
      enabled: enabled,
      toggled: widget.active,
      label: widget.label,
      child: enabled
          ? GestureDetector(onTap: widget.onTap, child: tile)
          : AbsorbPointer(
              child: Opacity(opacity: disabledOpacity, child: tile),
            ),
    );
  }
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
/// levels (the module hover, `ModuleHoverScope`: the buttons have no hover of their own); pressed a glyph turns
/// white `#f7f7f8`, without a pill.
const double mediaButtonSide = 30;
const List<double> mediaButtonCenters = [30, 70, 110];
const double mediaArtRadius = 8;
const Color mediaTitle = Color(0xE6FFFFFF);
const Color mediaSubtitle = Color(0x85FFFFFF);
const double mediaGlyphActive = 0.52;
const double mediaGlyphDisabled = 0.35;
const double mediaGlyphPressed = 0.96;

class _MediaState extends State<_Media> {
  final MediaPlayers _players = MediaPlayers.instance;
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
        : mediaGlyphActive;
    return Positioned(
      left: centerX - mediaButtonSide / 2,
      top: 111 - mediaButtonSide / 2,
      width: mediaButtonSide,
      height: mediaButtonSide,
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
    );
  }

  @override
  Widget build(BuildContext context) {
    final now = _players.now;
    final active = now != null;
    return Stack(
      children: [
        Positioned.fill(
          child: GestureDetector(
            behavior: HitTestBehavior.opaque,
            onTap: active ? widget.onOpen : null,
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
                      if (now.byline.isNotEmpty)
                        Text(
                          now.byline,
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
          transport: active && now.playing ? _Transport.pause : _Transport.play,
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
              if (now != null && now.byline.isNotEmpty)
                Text(
                  now.byline,
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
        if (enabled)
          Positioned(
            left:
                trackLeft +
                sliderFill(value, length: _length) -
                knobSize.width / 2,
            top: trackCenter - knobSize.height / 2,
            width: knobSize.width,
            height: knobSize.height,
            child: const IgnorePointer(child: _Knob()),
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

/// The privacy pill (SH-CC-25): a 24 pt capsule of the modules' glass, centred over the stack;
/// a 15 pt `#6e7cff` rounded square with the microphone or camera, 11 pt from the left; the
/// app, bold `#f0f0f1`, 6 pt after it ("<app> recently" once it stopped). A click opens the
/// privacy settings (provisional: the reference's click was not observed).
class _PrivacyPill extends StatelessWidget {
  const _PrivacyPill({required this.use, required this.onTap});

  final PrivacyUse use;
  final VoidCallback? onTap;

  @override
  Widget build(BuildContext context) {
    final l = l10nOf(context);
    final text = use.inUse
        ? l.ccPrivacyNow(use.app)
        : l.ccPrivacyRecent(use.app);
    return Semantics(
      button: onTap != null,
      label: text,
      child: GestureDetector(
        onTap: onTap,
        child: LiquidGlass(
          radius: pillHeight / 2,
          material: GlassMaterial.regular,
          interactive: false,
          child: Padding(
            padding: const EdgeInsets.only(left: 11, right: 14),
            child: Row(
              mainAxisSize: MainAxisSize.min,
              children: [
                Container(
                  width: 15,
                  height: 15,
                  decoration: BoxDecoration(
                    color: pillIcon,
                    borderRadius: BorderRadius.circular(4),
                  ),
                  child: Center(
                    child: Glyph(
                      use.kind == 'camera'
                          ? GlyphKind.camera
                          : GlyphKind.microphone,
                      color: const Color(0xFFFFFFFF),
                      size: 11,
                    ),
                  ),
                ),
                const SizedBox(width: 6),
                ConstrainedBox(
                  constraints: const BoxConstraints(maxWidth: 220),
                  child: Text(
                    text,
                    maxLines: 1,
                    overflow: TextOverflow.ellipsis,
                    style: _text(
                      titleSize,
                      weight: FontWeight.w700,
                    ).copyWith(color: pillText),
                  ),
                ),
              ],
            ),
          ),
        ),
      ),
    );
  }
}

/// The Edit Controls button: a 118 × 24 capsule of the modules' glass, centred, 16 pt under
/// the last module; white text, ~11–12 pt from each side.
const Size editButton = Size(118, 24);
const double editButtonGap = 16;

class _EditButton extends StatelessWidget {
  const _EditButton({required this.label, required this.onTap});

  final String label;
  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) => Semantics(
    button: true,
    label: label,
    child: GestureDetector(
      onTap: onTap,
      behavior: HitTestBehavior.opaque,
      child: LiquidGlass(
        radius: editButton.height / 2,
        material: GlassMaterial.regular,
        interactive: false,
        child: Center(
          child: Text(
            label,
            maxLines: 1,
            overflow: TextOverflow.ellipsis,
            style: _text(titleSize, weight: FontWeight.w600),
          ),
        ),
      ),
    ),
  );
}

/// The shadow of the module stack: drawn outside the modules only.
class _StackShadow {
  const _StackShadow({
    required this.origin,
    required this.height,
    required this.slots,
  });

  final Offset origin;
  final double height;
  final List<ModuleSlot> slots;

  Object get cacheKey => Object.hash(origin, height, Object.hashAll(slots));

  void paintShadow(Canvas canvas, Size size) {
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
}
