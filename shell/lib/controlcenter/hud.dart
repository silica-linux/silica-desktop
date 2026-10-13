// L'HUD del volume e della luminosità, da plan/reference/hud.md.
//
// Un pannello di vetro `regular` 292 × 64 in alto a destra, 10 pt sotto la barra dei menu e
// 10 pt dal bordo dello schermo, come un modulo del Centro di controllo: titolo (il nome
// dell'uscita audio, o "Schermo"), traccia di 4 pt con 17 tacche sotto (una per passo), icona
// a destra. Compare a ogni cambio di volume, muto o luminosità, salvo con il Centro di
// controllo aperto (lì cambia il cursore). Comparsa ~380 ms in scala da 0,91 e dissolvenza
// che accelera; permanenza 1,09 s dall'ultimo cambio (un cambio nuovo la fa ripartire senza
// ricomparire); scomparsa ~550 ms con un calo rapido nei primi ~80 ms e una coda lunga.

import 'dart:async';

import 'package:flutter/widgets.dart';
import 'package:silica_ui/silica_ui.dart';

import '../menubar/menubar.dart' show barHeight;
import '../system/system_status.dart';
import 'glyphs.dart';

const Size hudSize = Size(292, 64);
const double hudMargin = 10;
const double hudRadius = 28;
const double hudTitleLeft = 16;
const double hudTitleSize = 14;

/// Misure del riferimento (pannello a x 2258): piccolo altoparlante a 16–22 pt, traccia da
/// 31,5 a 251,5, tacche a 35,5 + i · 13,25, il riempimento finisce sulla tacca del livello
/// (35,5 + v · 212), grande altoparlante a 258–273, icone centrate a 41,5 pt dall'alto.
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

/// Le tacche: appena più chiare del fondo (`#86878e` sul riempimento `#6f7076`).
const Color hudDot = Color(0x33FFFFFF);

const Duration hudAppear = Duration(milliseconds: 380);
const Duration hudHold = Duration(milliseconds: 1090);
const Duration hudVanish = Duration(milliseconds: 550);
const double hudScaleFrom = 0.91;

/// Opacità e scala durante la comparsa (`t` 0–1): la dissolvenza accelera verso la fine, la
/// larghezza cresce quasi linearmente.
double hudAppearOpacity(double t) => (t * t).clamp(0.0, 1.0);
double hudAppearScale(double t) =>
    hudScaleFrom + (1 - hudScaleFrom) * t.clamp(0.0, 1.0);

/// Opacità e scala durante la scomparsa, `ms` dal suo inizio.
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

  /// Il Centro di controllo è aperto: i cambi si vedono nel suo cursore.
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
    // Una sorgente che diventa disponibile (la prima lettura dal daemon) non è un cambio
    // dell'utente: servono `available` prima e dopo.
    if (last == null || widget.suppressed) return;
    final audio =
        now.audioAvailable &&
        last.audioAvailable &&
        (now.volume != last.volume || now.muted != last.muted);
    final screen =
        now.brightnessAvailable &&
        last.brightnessAvailable &&
        now.brightness != last.brightness;
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
        ? (s.audioName.isNotEmpty ? s.audioName : 'Uscita audio')
        : 'Schermo';
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
                    // Come i moduli: il bordo scuro dei lati 1 pt fuori dal pannello.
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

  /// Null: muto, la traccia resta vuota.
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
