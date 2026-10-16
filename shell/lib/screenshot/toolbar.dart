// The ⇧⌘5 capture bar (kit "App · Screenshot": `Apps/Screenshot/Capture Bar`, `Bar Capture
// Screen|Window`, `Bar Record Selection`): a capsule of 496 × 48 on Glass/Regular (the kit draws
// Thick; `materials.md` says Regular for the Screenshot bar and the docs win), 8 pt above the Dock
// (README, "known differences"). Left to right, 8 pt from the edges: Close (a 32 pt toolbar
// icon button), two Large icon segmented controls (capture the screen, a window, a selection;
// record the screen, a window, a selection: 104 × 32 each, the chosen mode on the pill) with
// 1 pt separators in 8 pt slots, the Large pull-down for Options (96 × 32) and the Large primary
// push button (104 × 32), which reads Capture and is there only while there is a target.
//
// Behaviour kept from the measurements (`docs/spec/screenshot-ui.md`, SCREENSHOT-3): the six
// mode buttons have a tooltip without delay that stays after the pointer leaves; close, Options
// and the action button have none. In window mode the action button hides while the pointer is
// on the bar and the bar shrinks on the right. The bar is moved by dragging its background.
// The bar appears with `fade.quick` and an 8 pt rise on `spring.panel` and goes with the same
// (motion.md, Screenshot bar); the animation of the width is the measured one (60 ms, 450 ms).
//
// Recording is not available yet (#153, PipeWire screencast): its segmented control is drawn
// disabled (35 %) and does nothing.

import 'package:flutter/gestures.dart';
import 'package:flutter/physics.dart';
import 'package:flutter/widgets.dart';
import 'package:silica_ui/silica_ui.dart';

import '../system/glass_profile.dart';
import '../system/shadowed_glass.dart';
import 'settings.dart';

/// The bar with the action button; without it the bar is [toolbarActionSpace] narrower on
/// the right (the 104 pt button and the 4 pt before it).
const Size toolbarSize = Size(496, 48);
const double toolbarActionSpace = 108;

/// From the bottom of the screen to the bar's bottom edge: the Dock's shelf (60) and its
/// 16 pt margin, then 8 pt (kit: `Dock/Shelf` at 16 pt from the bottom of the screen, the
/// bar 8 pt above it).
const double toolbarAboveDock = 60 + 16 + 8;

/// The bar's capsule.
const double toolbarRadius = 24;

/// Where the controls sit, from the bar's left edge and top (kit `Bar Capture Screen`).
const double barInset = 8;
const double closeX = 8;
const double captureModesX = 44;
const double separator1X = 152;
const double recordModesX = 164;
const double separator2X = 272;
const double optionsX = 284;
const double actionX = 384;

/// A segmented control of three icons (Large, `Segments/Control/One 3 Icon/Large`): 104 × 32,
/// 4 pt inside the track, 32 pt per item.
const double modesWidth = 104;
const double controlHeight = 32;
const double modeItem = 32;
const double modeTrackInset = 4;
const double optionsWidth = 96;
const double actionWidth = 104;

/// The separator slot: 8 × 24 with a 1 × 16 line in it.
const double separatorSlot = 8;

/// Hiding or showing the action button: it starts ~60 ms after the pointer reaches or
/// leaves the bar, fades in 250 ms and the bar's right edge slides in 450 ms (measured).
const Duration actionDelay = Duration(milliseconds: 60);
const Duration actionFadeDuration = Duration(milliseconds: 250);
const Duration actionBarDuration = Duration(milliseconds: 450);

/// A mode tooltip is the Amalfi tooltip (Thick, radius 8) placed 8 pt above the bar
/// (`Tooltips/Placement`), centred on its button.
const double tipAboveBar = AmalfiTooltipMetrics.distance;
const double tipHeight = 24;

/// The Options menu: 208 wide (kit `Options Menu Panel`), opening upwards with its left edge
/// on the Options button's and its bottom 8 pt above the bar, or below the bar when there is
/// no room above.
const double optionsMenuWidth = 208;
const double optionsMenuGap = 8;

/// The icons of the modes (Icon Kit `symbols/regular`).
const List<String> captureSymbols = ['display', 'window', 'selection'];
const List<String> recordSymbols = ['video', 'screen-share', 'record-circle'];

/// The tooltip of the six mode buttons of the bar (localized): close, Options and the action
/// button have none (SCREENSHOT-3).
@immutable
class ShotBarTips {
  const ShotBarTips({required this.capture, required this.record});

  /// Screen, window, selection.
  final List<String> capture;
  final List<String> record;
}

/// The tooltip of a mode button: Callout on Glass/Thick (the compositor's `tooltip` kind).
class ShotTip extends StatelessWidget {
  const ShotTip({super.key, required this.text});

  final String text;

  @override
  Widget build(BuildContext context) {
    final colors = AmalfiTheme.of(context).colors;
    return ShadowedGlass(
      key: const ValueKey('bar tooltip box'),
      profile: ShellGlassProfile.tooltip,
      radius: AmalfiTooltipMetrics.radius,
      thick: true,
      child: Padding(
        padding: AmalfiTooltipMetrics.padding,
        child: Text(
          text,
          maxLines: 1,
          softWrap: false,
          style: AmalfiType.callout(color: colors.textPrimary),
        ),
      ),
    );
  }
}

/// The bar itself; its place is decided by whoever hosts it. Its width follows
/// [showAction]: [toolbarSize] with the action button, [toolbarActionSpace] less without it.
class ShotToolbar extends StatefulWidget {
  const ShotToolbar({
    super.key,
    required this.choice,
    required this.optionsLabel,
    required this.optionsOpen,
    required this.actionLabel,
    required this.closeLabel,
    required this.tips,
    required this.showAction,
    required this.onChoose,
    required this.onClose,
    required this.onOptions,
    required this.onAction,
    required this.onMove,
    required this.onMoved,
    this.closing = false,
    this.onClosed,
  });

  final ToolbarChoice choice;
  final String optionsLabel;

  /// The Options menu is open: its button keeps the pressed look.
  final bool optionsOpen;
  final String actionLabel;
  final String closeLabel;
  final ShotBarTips tips;

  /// The action button exists only while there is a target (K-05): in window mode it hides
  /// while the pointer is on the bar. Changing it animates the button and the bar's width.
  final bool showAction;
  final ValueChanged<ToolbarChoice> onChoose;
  final VoidCallback onClose;
  final VoidCallback onOptions;
  final VoidCallback onAction;

  /// The background is dragged: the bar moves by this much.
  final ValueChanged<Offset> onMove;
  final VoidCallback onMoved;

  /// The bar is going (× or Esc): it fades and sinks, then [onClosed] is called.
  final bool closing;
  final VoidCallback? onClosed;

  @override
  State<ShotToolbar> createState() => _ShotToolbarState();
}

class _ShotToolbarState extends State<ShotToolbar>
    with TickerProviderStateMixin {
  /// The mode button whose tooltip is shown: 0..2 capture, 3..5 record. It stays after the
  /// pointer leaves, until another mode button is hovered or the bar goes.
  int? _tip;

  /// 0 → 1: the fade (`fade.quick`) and the rise on `spring.panel`.
  late final AnimationController _fade = AnimationController(
    vsync: this,
    duration: AmalfiFades.quick.duration,
  );
  late final AnimationController _rise = AnimationController.unbounded(
    vsync: this,
    value: 0,
  );

  @override
  void initState() {
    super.initState();
    _fade.forward();
    _rise.animateWith(SpringSimulation(AmalfiSprings.panel, 0, 1, 0));
  }

  @override
  void didUpdateWidget(ShotToolbar old) {
    super.didUpdateWidget(old);
    if (widget.closing && !old.closing) {
      _fade.reverse().whenComplete(() => widget.onClosed?.call());
      _rise.animateWith(
        SpringSimulation(AmalfiSprings.panel, _rise.value, 0, 0),
      );
    }
  }

  @override
  void dispose() {
    _fade.dispose();
    _rise.dispose();
    super.dispose();
  }

  /// The mode button under `dx` (from the segmented control's left edge), or null on the
  /// track's padding.
  int? _itemAt(double dx) {
    final i = ((dx - modeTrackInset) / modeItem).floor();
    return i < 0 || i > 2 ? null : i;
  }

  Widget _modes({
    required double x,
    required List<String> symbols,
    required List<String> labels,
    required int tipBase,
    required Set<int> selected,
    ValueChanged<Set<int>>? onChanged,
  }) => Positioned(
    left: x,
    top: barInset,
    width: modesWidth,
    height: controlHeight,
    // The pointer on one of the three shows its tooltip at once, and it stays.
    child: MouseRegion(
      onEnter: (e) => _hover(e.localPosition, tipBase),
      onHover: (e) => _hover(e.localPosition, tipBase),
      child: AmalfiSegmentedControl(
        segments: [
          for (var i = 0; i < 3; i++)
            AmalfiSegment.icon(
              AmalfiSymbol(symbols[i]),
              semanticLabel: labels[i],
            ),
        ],
        selected: selected,
        onChanged: onChanged,
        size: AmalfiControlSize.large,
        // Glass is never stacked on glass: the bar is the compositor's, the track an
        // opaque fill over it.
        opaque: true,
      ),
    ),
  );

  void _hover(Offset at, int base) {
    final i = _itemAt(at.dx);
    if (i != null && _tip != base + i) setState(() => _tip = base + i);
  }

  @override
  Widget build(BuildContext context) {
    final colors = AmalfiTheme.of(context).colors;
    final tips = widget.tips;
    final tip = widget.optionsOpen ? null : _tip;
    final tipText = tip == null
        ? null
        : tip < 3
        ? tips.capture[tip]
        : tips.record[tip - 3];
    final tipX = tip == null
        ? 0.0
        : (tip < 3 ? captureModesX : recordModesX) +
              modeTrackInset +
              ((tip % 3) + 0.5) * modeItem;
    // The width: with the action button (shown) or without it, 450 ms after a 60 ms start.
    return AnimatedBuilder(
      animation: Listenable.merge([_fade, _rise]),
      builder: (context, child) => Opacity(
        opacity: AmalfiFades.quick.curve.transform(_fade.value.clamp(0, 1)),
        child: Transform.translate(
          offset: Offset(0, 8 * (1 - _rise.value)),
          child: child,
        ),
      ),
      child: TweenAnimationBuilder<double>(
        tween: Tween(end: widget.showAction ? 1 : 0),
        duration: actionBarDuration + actionDelay,
        curve: Interval(
          actionDelay.inMilliseconds /
              (actionBarDuration + actionDelay).inMilliseconds,
          1,
          curve: Curves.easeInOut,
        ),
        builder: (context, open, _) => SizedBox(
          width: toolbarSize.width - toolbarActionSpace * (1 - open),
          height: toolbarSize.height,
          child: Stack(
            clipBehavior: Clip.none,
            children: [
              Positioned.fill(
                child: ShadowedGlass(
                  profile: ShellGlassProfile.control,
                  radius: toolbarRadius,
                  child: SizedBox.expand(
                    key: const ValueKey('screenshot bar'),
                    child: Stack(
                      children: [
                        // The background: dragging it moves the bar.
                        Positioned.fill(
                          child: GestureDetector(
                            key: const ValueKey('bar background'),
                            behavior: HitTestBehavior.opaque,
                            dragStartBehavior: DragStartBehavior.down,
                            onPanUpdate: (d) => widget.onMove(d.delta),
                            onPanEnd: (_) => widget.onMoved(),
                          ),
                        ),
                        Positioned(
                          left: closeX,
                          top: barInset,
                          child: AmalfiToolbarButton(
                            key: const ValueKey('close bar'),
                            icon: const AmalfiGlyph(AmalfiGlyphs.xmark),
                            semanticLabel: widget.closeLabel,
                            onPressed: widget.onClose,
                          ),
                        ),
                        _modes(
                          x: captureModesX,
                          symbols: captureSymbols,
                          labels: tips.capture,
                          tipBase: 0,
                          selected: {widget.choice.index},
                          onChanged: (s) {
                            if (s.isEmpty) return;
                            widget.onChoose(ToolbarChoice.values[s.first]);
                          },
                        ),
                        _separator(separator1X, colors),
                        // TODO(#153): screen recording; until then disabled, no action.
                        _modes(
                          x: recordModesX,
                          symbols: recordSymbols,
                          labels: tips.record,
                          tipBase: 3,
                          selected: const {},
                        ),
                        _separator(separator2X, colors),
                        Positioned(
                          left: optionsX,
                          top: barInset,
                          child: ShotOptionsButton(
                            key: const ValueKey('options'),
                            label: widget.optionsLabel,
                            open: widget.optionsOpen,
                            onTap: widget.onOptions,
                          ),
                        ),
                        _action(),
                      ],
                    ),
                  ),
                ),
              ),
              if (tipText != null)
                // Centred on the button, its bottom 8 pt over the bar's top edge.
                Positioned(
                  left: tipX - 200,
                  width: 400,
                  top: -tipAboveBar - tipHeight,
                  height: tipHeight,
                  child: IgnorePointer(
                    child: Center(
                      child: ShotTip(
                        key: const ValueKey('bar tooltip'),
                        text: tipText,
                      ),
                    ),
                  ),
                ),
            ],
          ),
        ),
      ),
    );
  }

  Widget _separator(double x, AmalfiColors colors) => Positioned(
    left: x,
    top: 12,
    width: separatorSlot,
    height: 24,
    child: Center(
      child: SizedBox(
        width: 1,
        height: 16,
        child: ColoredBox(color: colors.lineSeparator),
      ),
    ),
  );

  /// The action button: 104 × 32, [barInset] from the bar's top, bottom and right edges; it
  /// fades in 250 ms (after the same 60 ms start) as the bar's width changes.
  Widget _action() {
    return Positioned(
      left: actionX,
      top: barInset,
      width: actionWidth,
      height: controlHeight,
      child: TweenAnimationBuilder<double>(
        tween: Tween(end: widget.showAction ? 1 : 0),
        duration: actionFadeDuration + actionDelay,
        curve: Interval(
          actionDelay.inMilliseconds /
              (actionFadeDuration + actionDelay).inMilliseconds,
          1,
        ),
        builder: (context, opacity, _) {
          if (opacity == 0) return const SizedBox.shrink();
          return IgnorePointer(
            ignoring: !widget.showAction,
            child: Opacity(
              opacity: opacity,
              child: SizedBox(
                key: const ValueKey('capture fill'),
                width: actionWidth,
                height: controlHeight,
                child: AmalfiPushButton(
                  key: const ValueKey('capture'),
                  label: widget.actionLabel,
                  variant: AmalfiPushButtonVariant.primary,
                  size: AmalfiControlSize.large,
                  onPressed: widget.onAction,
                ),
              ),
            ),
          );
        },
      ),
    );
  }
}

/// The Options pull-down (`Buttons/Pull-down/Large`): 96 × 32, `Surface/Control` capsule with
/// its border and soft shadow, the label in Body and a chevron; pressed (or with its menu open)
/// `Surface/Control Pressed`. The menu itself is [ShotMenu]: silica_ui's pull-down opens a plain
/// list, and this one has sections, checkmarks and a place of its own.
class ShotOptionsButton extends StatefulWidget {
  const ShotOptionsButton({
    super.key,
    required this.label,
    required this.onTap,
    this.open = false,
  });

  final String label;
  final VoidCallback onTap;
  final bool open;

  @override
  State<ShotOptionsButton> createState() => _ShotOptionsButtonState();
}

class _ShotOptionsButtonState extends State<ShotOptionsButton> {
  bool _pressed = false;
  bool _hovered = false;

  @override
  Widget build(BuildContext context) {
    final colors = AmalfiTheme.of(context).colors;
    final pressed = _pressed || widget.open;
    final shape = AmalfiSmoothBorder(
      radius: AmalfiRadii.capsule(controlHeight),
      side: BorderSide(
        color: colors.lineBorder,
        strokeAlign: BorderSide.strokeAlignInside,
      ),
    );
    return Semantics(
      button: true,
      label: widget.label,
      child: MouseRegion(
        onEnter: (_) => setState(() => _hovered = true),
        onExit: (_) => setState(() => _hovered = false),
        child: GestureDetector(
          behavior: HitTestBehavior.opaque,
          onTapDown: (_) => setState(() => _pressed = true),
          onTapCancel: () => setState(() => _pressed = false),
          onTapUp: (_) => setState(() => _pressed = false),
          onTap: widget.onTap,
          child: DecoratedBox(
            decoration: AmalfiShapeDecoration(
              color: pressed
                  ? colors.surfaceControlPressed
                  : colors.surfaceControl,
              shape: shape,
              shadows: amalfiControlShadows(colors),
            ),
            child: DecoratedBox(
              decoration: AmalfiShapeDecoration(
                color: _hovered && !pressed ? colors.accentAmalfiSoft : null,
                shape: shape,
              ),
              child: SizedBox(
                width: optionsWidth,
                height: controlHeight,
                child: Padding(
                  padding: const EdgeInsets.only(left: 12, right: 8),
                  child: Row(
                    children: [
                      Expanded(
                        child: Text(
                          widget.label,
                          maxLines: 1,
                          softWrap: false,
                          overflow: TextOverflow.ellipsis,
                          style: AmalfiType.body(color: colors.textPrimary),
                        ),
                      ),
                      const SizedBox(width: 4),
                      AmalfiGlyph(
                        AmalfiGlyphs.chevronDown,
                        size: 16,
                        color: colors.textSecondary,
                      ),
                    ],
                  ),
                ),
              ),
            ),
          ),
        ),
      ),
    );
  }
}
