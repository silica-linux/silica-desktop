// The volume and brightness HUD, from plan/reference/hud.md (the same in light and dark).
//
// A 292 × 64 `regular` glass panel at the top right, 10 pt below the menu bar and 10 pt from
// the screen edge, like a Control Center module: title (the name of the audio output, or
// "Schermo"), a 4 pt track with 17 ticks below (one per step), icon on the right. It appears
// at every volume or mute change and at every brightness requested by Silica (keys,
// commands; not adaptive brightness, which changes the value by itself), except with the
// Control Center open (there the slider changes). Appearance ~380 ms scaling from 0.91 with
// an accelerating fade; it stays 1.09 s from the last change (a new change restarts it
// without reappearing); disappearance ~550 ms with a fast drop in the first ~80 ms and a long tail.

import 'dart:async';

import 'package:flutter/widgets.dart';
import 'package:silica_ui/silica_ui.dart';

import '../menubar/menubar.dart' show barHeight;
import '../l10n/l10n.dart';
import '../system/system_status.dart';
import 'glyphs.dart';

const Size hudSize = Size(292, 64);
const double hudMargin = 10;
const double hudRadius = 28;
const double hudTitleLeft = 16;
const double hudTitleSize = 14;

/// Reference measurements (panel at x 2258): small speaker at 16–22 pt, track from 31.5 to
/// 251.5, ticks at 35.5 + i · 13.25, the fill ends on the tick of the level
/// (35.5 + v · 212), big speaker at 258–273, icons centered 41.5 pt from the top.
const double hudTrackLeft = 31.5;
const double hudTrackLength = 220;
const double hudFillStart = 4;
const double hudFillSpan = 212;
const double hudIconCenterY = 41.5;
const double hudTrackTop = 39;
const double hudTrackHeight = 4;
const double hudDotsTop = 46;
const double hudDotsLeft = 35.5;
const double hudDotsPitch = 13.25;
const int hudDots = 17;
const Color hudTrackRest = Color(0xFF84858B);

/// The ticks: just lighter than the background (`#86878e` over the `#6f7076` fill).
const Color hudDot = Color(0x33FFFFFF);

const Duration hudAppear = Duration(milliseconds: 380);
const Duration hudHold = Duration(milliseconds: 1090);
const Duration hudVanish = Duration(milliseconds: 550);
const double hudScaleFrom = 0.91;

/// Opacity and scale during the appearance (`t` 0–1): the fade accelerates towards the end,
/// the width grows almost linearly.
double hudAppearOpacity(double t) => (t * t).clamp(0.0, 1.0);
double hudAppearScale(double t) =>
    hudScaleFrom + (1 - hudScaleFrom) * t.clamp(0.0, 1.0);

/// Opacity and scale during the disappearance, `ms` from its start.
double hudVanishOpacity(double ms) {
  if (ms <= 0) return 1;
  if (ms < 80) return 1 - 0.78 * ms / 80;
  final rest = (1 - (ms - 80) / (hudVanish.inMilliseconds - 80)).clamp(
    0.0,
    1.0,
  );
  return 0.22 * rest * rest;
}

double hudVanishScale(double ms) =>
    1 - (1 - hudScaleFrom) * (ms / 410).clamp(0.0, 1.0);

enum HudKind { volume, brightness }

class SystemHud extends StatefulWidget {
  const SystemHud({super.key, required this.system, required this.suppressed});

  final SystemSource system;

  /// The Control Center is open: the changes show in its slider.
  final bool suppressed;

  @override
  State<SystemHud> createState() => _SystemHudState();
}

class _SystemHudState extends State<SystemHud> with TickerProviderStateMixin {
  late final AnimationController _appear;
  late final AnimationController _vanish;
  SystemSnapshot? _last;
  HudKind _kind = HudKind.volume;
  bool _visible = false;
  Timer? _hold;

  @override
  void initState() {
    super.initState();
    void repaint() => setState(() {});
    _appear = AnimationController(vsync: this, duration: hudAppear)
      ..addListener(repaint);
    _vanish = AnimationController(vsync: this, duration: hudVanish)
      ..addListener(repaint);
    _last = widget.system.value;
    widget.system.addListener(_changed);
  }

  @override
  void dispose() {
    widget.system.removeListener(_changed);
    _hold?.cancel();
    _appear.dispose();
    _vanish.dispose();
    super.dispose();
  }

  void _changed() {
    final now = widget.system.value;
    final last = _last;
    _last = now;
    // A source that becomes available (the first read from the daemon) is not a user
    // change: `available` is needed before and after.
    if (last == null || widget.suppressed) return;
    final audio =
        now.audioAvailable &&
        last.audioAvailable &&
        (now.volume != last.volume || now.muted != last.muted);
    // Only the brightness requested by the user: adaptive brightness changes the value without the HUD.
    final screen =
        now.brightnessAvailable &&
        last.brightnessAvailable &&
        now.brightnessRequests != last.brightnessRequests;
    if (!audio && !screen) return;
    _show(audio ? HudKind.volume : HudKind.brightness);
  }

  void _show(HudKind kind) {
    setState(() => _kind = kind);
    if (!_visible || _vanish.isAnimating) {
      _visible = true;
      _vanish.value = 0;
      _appear.forward(from: 0);
    }
    _hold?.cancel();
    _hold = Timer(hudHold, _hide);
  }

  Future<void> _hide() async {
    await _vanish.forward(from: 0);
    if (mounted && _vanish.value >= 1) setState(() => _visible = false);
  }

  @override
  Widget build(BuildContext context) {
    if (!_visible) return const SizedBox.shrink();
    final vanishing = _vanish.value > 0;
    final vanishMs = _vanish.value * hudVanish.inMilliseconds;
    final opacity = vanishing
        ? hudVanishOpacity(vanishMs)
        : hudAppearOpacity(_appear.value);
    final scale = vanishing
        ? hudVanishScale(vanishMs)
        : hudAppearScale(_appear.value);
    final s = widget.system.value;
    final volume = _kind == HudKind.volume;
    final value = volume ? (s.muted ? null : s.volume) : s.brightness;
    final title = volume
        ? (s.audioName.isNotEmpty
              ? s.audioName
              : l10nOf(context).hudAudioOutput)
        : l10nOf(context).ccDisplay;
    return LayoutBuilder(
      builder: (context, constraints) => Stack(
        children: [
          Positioned(
            left: constraints.maxWidth - hudMargin - hudSize.width,
            top: barHeight + hudMargin,
            width: hudSize.width,
            height: hudSize.height,
            child: Opacity(
              opacity: opacity,
              child: Transform.scale(
                scale: scale,
                child: Stack(
                  clipBehavior: Clip.none,
                  children: [
                    // Like the modules: the dark side edge 1 pt outside the panel.
                    const Positioned(
                      left: -1,
                      right: -1,
                      top: 0,
                      bottom: 0,
                      child: Glass(
                        radius: hudRadius,
                        material: GlassMaterial.regular,
                      ),
                    ),
                    Positioned(
                      left: hudTitleLeft,
                      right: 40,
                      top: 10,
                      child: Text(
                        title,
                        maxLines: 1,
                        overflow: TextOverflow.ellipsis,
                        style: TextStyle(
                          fontFamily: uiFontFamily,
                          package: 'silica_ui',
                          fontSize: hudTitleSize,
                          fontWeight: FontWeight.w600,
                          color: const Color(0xFFFFFFFF),
                        ),
                      ),
                    ),
                    Positioned(
                      left: hudTrackLeft,
                      top: hudTrackTop,
                      width: hudTrackLength,
                      height: hudTrackHeight,
                      child: CustomPaint(painter: _HudTrack(value)),
                    ),
                    for (var i = 0; i < hudDots; i++)
                      Positioned(
                        left: hudDotsLeft + i * hudDotsPitch - 1,
                        top: hudDotsTop,
                        width: 2,
                        height: 2,
                        child: const ColoredBox(color: hudDot),
                      ),
                    Positioned(
                      left: 13,
                      top: hudIconCenterY - 6.5,
                      width: 13,
                      height: 13,
                      child: Glyph(
                        volume ? GlyphKind.speakerSmall : GlyphKind.sunSmall,
                        color: const Color(0xFFFFFFFF),
                        size: 13,
                      ),
                    ),
                    Positioned(
                      left: 258,
                      top: hudIconCenterY - 8,
                      width: 16,
                      height: 16,
                      child: Glyph(
                        volume ? GlyphKind.speakerLarge : GlyphKind.sunLarge,
                        color: const Color(0xFFFFFFFF),
                        size: 16,
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
  }
}

class _HudTrack extends CustomPainter {
  const _HudTrack(this.value);

  /// Null: muted, the track stays empty.
  final double? value;

  @override
  void paint(Canvas canvas, Size size) {
    final radius = Radius.circular(size.height / 2);
    canvas.drawRRect(
      RRect.fromRectAndRadius(Offset.zero & size, radius),
      Paint()..color = hudTrackRest,
    );
    final v = value;
    if (v == null || v <= 0) return;
    canvas.drawRRect(
      RRect.fromRectAndRadius(
        Rect.fromLTWH(
          0,
          0,
          hudFillStart + hudFillSpan * v.clamp(0, 1),
          size.height,
        ),
        radius,
      ),
      Paint()..color = const Color(0xFFFFFFFF),
    );
  }

  @override
  bool shouldRepaint(_HudTrack old) => old.value != value;
}
