// The volume and brightness HUD on the Amalfi materials (materials.md: HUDs are Glass/Regular).
//
// A 292 x 64 Glass/Regular panel (radius 20) at the top right, 10 pt below the menu bar and
// 10 pt from the screen edge, laid out like a Control Center slider module: title (Headline: the
// name of the audio output, or the display label), a 4 pt `Surface/Sunken` track filled with
// `Accent/Amalfi` (no thumb: the HUD is not interactive), a 16 pt glyph at each end. It appears
// at every volume or mute change and at every brightness requested by Silica (keys, commands;
// not adaptive brightness, which changes the value by itself), except with the Control Center
// open (there the slider changes). It stays 1.09 s from the last change (a new change restarts
// it without reappearing). Motion (motion.md): in with `fade.quick` and an 8 pt rise with
// `spring.panel`, out with `fade.quick`.

import 'dart:async';

import 'package:flutter/widgets.dart';
import 'package:silica_ui/silica_ui.dart';

import '../menubar/menubar.dart' show barHeight;
import '../l10n/l10n.dart';
import '../system/glass_profile.dart';
import '../system/system_status.dart';
import 'glyphs.dart';
import 'palette.dart';

const Size hudSize = Size(292, 64);
const double hudMargin = 10;
const double hudRadius = 20;
const double hudTitleLeft = 16;

/// Same layout as the Display module: glyphs 16 pt at x 16 and 260, the track from x 40 to 252
/// at y 44.
const double hudTrackLeft = 40;
const double hudTrackLength = 212;
const double hudTrackHeight = 4;
const double hudTrackTop = 42;
const double hudGlyphTop = 36;

/// Measured (hud.md "Animation"): appears over ~380 ms scaling from 0.91 to 1 around its
/// centre, the opacity accelerating towards the end, no overshoot; stays ~1.09 s after the last
/// key; hides over ~550 ms, a quick drop in the first 80 ms and then a long slow tail.
const Duration hudAppear = Duration(milliseconds: 380);
const Duration hudHold = Duration(milliseconds: 1090);
const Duration hudVanish = Duration(milliseconds: 550);
const double hudScaleFrom = 0.91;

/// Opacity and scale during the appearance (`t` 0-1): the fade accelerates towards the end,
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

enum HudKind { volume, brightness, keyboard }

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
    // The keyboard backlight: like the screen, only when Silica set it (keys, slider).
    final keyboard =
        now.keyboardAvailable &&
        last.keyboardAvailable &&
        now.keyboardRequests != last.keyboardRequests;
    if (!audio && !screen && !keyboard) return;
    _show(
      audio
          ? HudKind.volume
          : screen
          ? HudKind.brightness
          : HudKind.keyboard,
    );
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
    final palette = CcPalette.of(context);
    final volume = _kind == HudKind.volume;
    final keyboard = _kind == HudKind.keyboard;
    final value = switch (_kind) {
      HudKind.volume => s.muted ? null : s.volume,
      HudKind.brightness => s.brightness,
      HudKind.keyboard => s.keyboardBrightness,
    };
    final title = switch (_kind) {
      HudKind.volume =>
        s.audioName.isNotEmpty ? s.audioName : l10nOf(context).hudAudioOutput,
      HudKind.brightness => l10nOf(context).ccDisplay,
      // Not measured on the reference (no backlit keyboard there): the Display layout.
      HudKind.keyboard => l10nOf(context).ccKeyboardBrightness,
    };
    final small = volume
        ? GlyphKind.speakerSmall
        : keyboard
        ? GlyphKind.keyboard
        : GlyphKind.sunSmall;
    final large = volume
        ? GlyphKind.speakerLarge
        : keyboard
        ? GlyphKind.keyboard
        : GlyphKind.sunLarge;
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
                    const Positioned.fill(
                      child: ProfiledGlass(
                        profile: ShellGlassProfile.control,
                        radius: hudRadius,
                        material: GlassMaterial.regular,
                      ),
                    ),
                    Positioned(
                      left: hudTitleLeft,
                      right: 16,
                      top: 8,
                      child: Text(
                        title,
                        maxLines: 1,
                        overflow: TextOverflow.ellipsis,
                        style: AmalfiType.headline(color: palette.text),
                      ),
                    ),
                    Positioned(
                      left: hudTitleLeft,
                      top: hudGlyphTop,
                      child: CcGlyph(small, color: palette.text, size: 16),
                    ),
                    // The read-only slider, no thumb, centred on the 4 pt track's line.
                    Positioned(
                      left: hudTrackLeft,
                      top: hudTrackTop + hudTrackHeight / 2 - 10,
                      width: hudTrackLength,
                      height: 20,
                      child: AmalfiSlider.indicator(
                        key: const ValueKey('hud level'),
                        // Muted: the track stays empty.
                        value: (value ?? 0).clamp(0.0, 1.0),
                      ),
                    ),
                    Positioned(
                      left: hudTrackLeft + hudTrackLength + 8,
                      top: hudGlyphTop,
                      child: CcGlyph(large, color: palette.text, size: 16),
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
