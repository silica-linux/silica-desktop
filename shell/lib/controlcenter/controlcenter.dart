// Il Centro di controllo, da plan/reference/controlcenter.md (aspetto chiaro).
//
// Una pila di moduli di vetro `regular` separati, ancorata in alto a destra: due colonne
// da 140 pt a 12 pt l'una dall'altra, 14 pt dal bordo destro, il primo modulo a y 42;
// righe da 64 pt a passo 76. Capsule 140 × 64 (raggio 32) con un disco da 36 pt a 14 pt dal
// bordo e il testo a 56 pt; il modulo multimediale 140 × 140 (raggio 35); Schermo e Suono
// 292 × 64 (raggio ~28) con un cursore senza manopola. Ombra ampia e morbida attorno alla
// pila; apertura e chiusura con le opacità e la scala misurate fotogramma per fotogramma.
//
// Moduli con un effetto vero: Wi-Fi, Bluetooth, Non disturbare, Schermo, Suono (da
// silica-daemon), la batteria se c'è. Clic sul disco dell'icona: accende o spegne; clic sul
// testo (o sul titolo dei cursori): il pannello di dettaglio (`detail.dart`), dove c'è; con
// il puntatore sopra un modulo con dettaglio compare la freccia ">". Esc dal dettaglio torna
// al Centro. Non c'è ancora "Modifica controlli" né il menu del clic destro.
//
// Il componente resta in esecuzione su un layer overlay a schermo intero: chiuso non disegna
// niente e non prende né puntatore né tastiera (silica/layer).

import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';
import 'package:silica_ui/silica_ui.dart';

import '../system/layer_control.dart';
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

/// Disco delle capsule: 36 pt, a 14 pt dal bordo; testo a 56 pt.
const double disc = 36;
const double discInset = 14;
const double textLeft = 56;

/// Disco attivo bianco con glifo blu, inattivo grigio freddo con glifo bianco.
const Color discOn = Color(0xFFFFFFFF);
const Color discOff = Color(0xFF8F9094);
const Color glyphOn = Color(0xFF007AFF);
const Color glyphOff = Color(0xFFFFFFFF);

/// Titoli (maiuscole 9 pt → ~12,4 pt) e sottotitoli (8 pt → ~11 pt), bianchi.
const double titleSize = 12.4;
const double subtitleSize = 11;
const Color textColor = Color(0xFFFFFFFF);

/// Cursori: traccia di 4 pt, riempimento bianco fino a `x0 + 10 + v · (L − 20)`, resto
/// `#7e7f84`; traccia da 36 a 255 pt dal bordo del modulo, icone a 15 e 260.
const double trackLeft = 36;
const double trackLength = 219;
const double trackHeight = 4;
const double trackCenter = 42;
const Color trackRest = Color(0xFF7E7F84);

/// Ombra della pila: −10 livelli sul bordo sinistro, nulla a ~130 pt, ~−12 fra la pila e
/// il bordo destro dello schermo e sopra: una gaussiana di σ ~65 pt di un rettangolo che
/// va dalla pila al bordo destro e dall'alto dello schermo al fondo della pila.
const Color shadowColor = Color(0x2A000000);
const double shadowSigma = 65;

/// Opacità durante l'apertura e la chiusura (ms dal primo fotogramma cambiato; il primo
/// punto è un fotogramma prima). controlcenter.md, "Animazioni".
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

/// Scala del contenuto: 1,0075 → 1 in ~107 ms all'apertura, 1 → 1,0075 in ~120 ms alla
/// chiusura, attorno al centro della pila.
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

/// Valore di un cursore per un punto a `dx` dal bordo sinistro della traccia.
double sliderValue(double dx) =>
    ((dx - 10) / (trackLength - 20)).clamp(0.0, 1.0);

/// Fine del riempimento di un cursore, dal bordo sinistro della traccia.
double sliderFill(double value) => 10 + value.clamp(0, 1) * (trackLength - 20);

/// Un modulo: rettangolo relativo alla pila e raggio.
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

/// La disposizione dei moduli presenti, dall'alto (coordinate relative alla pila).
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
  slots['dnd'] = const ModuleSlot(
    Rect.fromLTWH(0, rowStep * 2, columnWidth, rowHeight),
    capsuleRadius,
  );
  if (s.batteryPresent) {
    slots['battery'] = const ModuleSlot(
      Rect.fromLTWH(right, rowStep * 2, columnWidth, rowHeight),
      capsuleRadius,
    );
  }
  var y = rowStep * 3;
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

/// I moduli con un pannello di dettaglio.
const Set<String> detailModules = {'wifi', 'sound', 'display'};

/// Spazio attorno alla pila per il pannello di dettaglio, che sporge a sinistra e in alto.
const Offset detailPad = Offset(-detailLeft, -detailTop);

class _ControlCenterState extends State<ControlCenter>
    with TickerProviderStateMixin {
  late final AnimationController _anim;

  /// Il dettaglio aperto (o in apertura o chiusura) e la sua trasformazione (valore = ms).
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
    _commands?.cancel();
    _anim.dispose();
    _detailAnim.dispose();
    _focus.dispose();
    super.dispose();
  }

  void _changed() => setState(() {});

  void _toggle() => _open ? _hide() : _show();

  void _show() {
    setState(() => _open = true);
    LayerControl.inputRegion(null);
    LayerControl.keyboard('exclusive');
    _anim.duration = openDuration;
    _anim.forward(from: 0);
    WidgetsBinding.instance.addPostFrameCallback((_) => _focus.requestFocus());
  }

  void _detailTick() {
    // Finito il ritorno, il dettaglio sparisce.
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
      _ => const DetailData(),
    };
    if (mounted && _detail == key) setState(() => _detailData = data);
  }

  void _closeDetail() {
    if (_detail == null || _detailClosing) return;
    setState(() => _detailClosing = true);
    _detailAnim.forward(from: 0);
  }

  /// Quanto del modulo `_detail` è diventato pannello (0–1).
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
    LayerControl.keyboard('none');
    LayerControl.inputRegion(const []);
    _anim.duration = closeDuration;
    _anim.forward(from: 0);
  }

  @override
  Widget build(BuildContext context) => Stack(
    children: [
      Positioned.fill(child: _center(context)),
      // L'HUD del volume e della luminosità: non con il Centro aperto.
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
    // Il dettaglio: rettangolo e raggio fra il modulo e il pannello, e il contenuto.
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
              // Dal dettaglio si torna al Centro; dal Centro si chiude.
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
                  // L'ombra cambia solo con la disposizione: si disegna una volta, poi si
                  // cambia soltanto l'opacità del suo strato.
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
                        // Il vetro sporge di 1 pt a sinistra e a destra del riquadro del
                        // modulo (la riga scura dei lati cade fuori), non sopra e sotto.
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

  /// Il vetro di un modulo: 1 pt in più a sinistra e a destra.
  Rect _glassRect(Rect r) =>
      Rect.fromLTRB(r.left - 1, r.top, r.right + 1, r.bottom);

  /// Il contenuto del dettaglio aperto.
  DetailContent? _detailContent(SystemSnapshot s) {
    final system = widget.system;
    Widget track(double? v) => CustomPaint(painter: _Track(v));
    return switch (_detail) {
      'wifi' => wifiDetail(
        s: s,
        networks: _detailData.networks,
        othersOpen: _othersOpen,
        onSwitch: system.setWifi,
        onConnect: system.connectWifi,
        onToggleOthers: () => setState(() => _othersOpen = !_othersOpen),
      ),
      'sound' => soundDetail(
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
        s: s,
        onBrightness: system.setBrightness,
        track: track,
      ),
      _ => null,
    };
  }

  Widget _module(String key, SystemSnapshot s) {
    final system = widget.system;
    return switch (key) {
      'wifi' => _Capsule(
        glyph: GlyphKind.wifi,
        title: 'Wi-Fi',
        subtitle: !s.wifiAvailable
            ? 'Non disponibile'
            : !s.wifiEnabled
            ? 'Spento'
            : s.wifiSsid.isNotEmpty
            ? s.wifiSsid
            : s.wired
            ? 'Rete via cavo'
            : 'Non connesso',
        active: s.wifiAvailable && s.wifiEnabled,
        onTap: s.wifiAvailable ? () => system.setWifi(!s.wifiEnabled) : null,
        onOpen: s.wifiAvailable ? () => _openDetail('wifi') : null,
      ),
      'bluetooth' => _Capsule(
        glyph: GlyphKind.bluetooth,
        title: 'Bluetooth',
        subtitle: !s.bluetoothAvailable
            ? 'Non disponibile'
            : s.bluetoothPowered
            ? 'Acceso'
            : 'Spento',
        active: s.bluetoothAvailable && s.bluetoothPowered,
        onTap: s.bluetoothAvailable
            ? () => system.setBluetooth(!s.bluetoothPowered)
            : null,
      ),
      'dnd' => _Capsule(
        glyph: GlyphKind.moon,
        title: 'Non disturbare',
        subtitle: s.doNotDisturb ? 'Attivo' : 'Spento',
        active: s.doNotDisturb,
        onTap: () => system.setDoNotDisturb(!s.doNotDisturb),
      ),
      'battery' => _Capsule(
        glyph: GlyphKind.battery,
        glyphLevel: s.batteryPercent / 100,
        title: 'Batteria',
        subtitle:
            '${s.batteryPercent.round()}%${s.batteryCharging ? ' · in carica' : ''}',
        active: false,
      ),
      'media' => const _Media(),
      'display' => _SliderModule(
        onOpen: () => _openDetail('display'),
        title: 'Schermo',
        value: s.brightness,
        enabled: s.brightnessAvailable,
        low: GlyphKind.sunSmall,
        high: GlyphKind.sunLarge,
        onChanged: system.setBrightness,
      ),
      _ => _SliderModule(
        onOpen: () => _openDetail('sound'),
        title: 'Suono',
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
  });

  final GlyphKind glyph;
  final double glyphLevel;
  final String title;
  final String subtitle;
  final bool active;

  /// Il disco dell'icona: accende o spegne.
  final VoidCallback? onTap;

  /// Il testo: apre il dettaglio; senza dettaglio il testo fa come il disco.
  final VoidCallback? onOpen;

  @override
  State<_Capsule> createState() => _CapsuleState();
}

/// Freccia ">" dei moduli con dettaglio, al passaggio del puntatore: a destra, sopra il
/// testo (che si accorcia). Posizione e misura stimate dalla cattura (CC-01).
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
          // Testo: dettaglio (o, senza, lo stesso effetto del disco).
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
                    color: widget.active ? glyphOn : glyphOff,
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
                  // Come nel riferimento i testi lunghi vanno a capo ("Full immersion", il
                  // nome di una rete), al più due righe ciascuno.
                  Text(
                    widget.title,
                    maxLines: 2,
                    overflow: TextOverflow.ellipsis,
                    style: _text(titleSize, weight: FontWeight.w600),
                  ),
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

/// Il modulo multimediale senza riproduzione: la copertina vuota 40 × 40 a 14 pt dal bordo
/// e il testo in basso a sinistra (a 15 pt), passo di 15 pt.
class _Media extends StatelessWidget {
  const _Media();

  @override
  Widget build(BuildContext context) => Stack(
    children: [
      Positioned(
        left: 14,
        top: 14,
        width: 40,
        height: 40,
        child: DecoratedBox(
          decoration: BoxDecoration(
            color: const Color(0xFF7E7F84),
            borderRadius: BorderRadius.circular(10),
          ),
        ),
      ),
      Positioned(
        left: 15,
        right: 12,
        top: 64,
        child: Text(
          'Nessuna\nriproduzione',
          style: _text(
            titleSize,
            weight: FontWeight.w600,
          ).copyWith(height: 15 / titleSize),
        ),
      ),
    ],
  );
}

class _SliderModule extends StatelessWidget {
  const _SliderModule({
    this.onOpen,
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

  /// Il titolo apre il dettaglio.
  final VoidCallback? onOpen;

  void _set(Offset local) {
    if (enabled) onChanged(sliderValue(local.dx - trackLeft));
  }

  @override
  Widget build(BuildContext context) => GestureDetector(
    behavior: HitTestBehavior.opaque,
    onTapDown: (d) {
      // Sopra la traccia (il titolo): il dettaglio; sulla traccia: il valore.
      if (onOpen != null && d.localPosition.dy < trackCenter - 10) {
        onOpen!();
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
          width: trackLength,
          height: trackHeight,
          child: CustomPaint(painter: _Track(enabled ? value : null)),
        ),
        Positioned(
          left: 260,
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
  const _Track(this.value);

  /// Null: cursore non regolabile (solo la traccia, come nel riferimento).
  final double? value;

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
        Rect.fromLTWH(0, 0, sliderFill(v), size.height),
        radius,
      ),
      Paint()..color = const Color(0xFFFFFFFF),
    );
  }

  @override
  bool shouldRepaint(_Track old) => old.value != value;
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
          // Come il vetro: 1 pt in più a sinistra e a destra.
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

  @override
  bool shouldRepaint(_StackShadow old) =>
      old.origin != origin ||
      old.height != height ||
      !listEquals(old.slots, slots);
}
