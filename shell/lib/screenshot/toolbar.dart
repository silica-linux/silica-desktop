// The ⇧⌘5 bar (screenshot-ui.md, K-05/K-06, SCREENSHOT-3): glass 551 × 52 (466 without the
// action button), 4 pt above the Dock area, where it was last left. Left to right: Close (a
// disc with ×), capture the screen, a window, a selection (44 × 50 at a 48 pt pitch), a
// separator, record the screen, a window, a selection (at a 57 pt pitch), a separator,
// Options with an arrow (76 × 50) and the blue 80 × 36 action button, 8 pt from the bar's
// edges. The chosen mode sits on a rounded pill; the pointer over a mode that is not chosen
// changes nothing. The six mode buttons have a tooltip without delay that stays after the
// pointer leaves; close, Options and the action button have none. In window mode the action
// button hides while the pointer is on the bar and the bar shrinks 85 pt on the right. The
// bar is moved by dragging its background.
//
// Recording is not available yet (#153, PipeWire screencast): its three
// buttons are drawn dimmed and do nothing.

import '../system/glass_profile.dart';
import 'package:flutter/gestures.dart';
import 'package:flutter/widgets.dart';
import 'package:silica_ui/silica_ui.dart';

import '../system/cross_glyph.dart';
import 'settings.dart';

/// The bar with the action button; without it the bar is [toolbarActionSpace] narrower on
/// the right (SCREENSHOT-3: 551 and 466 pt).
const Size toolbarSize = Size(551, 52);
const double toolbarActionSpace = 85;

/// From the Dock area (84 pt exclusive zone) to the bar: 4 pt.
const double toolbarAboveDock = 84 + 4;

/// Where the controls sit, from the bar's left edge (AX frames, K-05, SCREENSHOT-3). The
/// separators are placed from the measured widths.
const double closeX = 13;
const List<double> captureX = [41, 89, 137];
const List<double> recordX = [203, 260, 317];
const double separator1X = 192;
const double separator2X = 372;
const double optionsX = 383;
const double actionX = 463;
const double modeWidth = 44;
const double optionsWidth = 76;
const double actionWidth = 80;

/// The action button (SCREENSHOT-3): 80 × 36, radius ~8, 8 pt from the bar's top, bottom and
/// right edges.
const double actionHeight = 36;
const double actionInset = 8;
const double actionRadius = 8;

/// Hiding or showing the action button: it starts ~60 ms after the pointer reaches or
/// leaves the bar, fades in 250 ms and the bar's right edge slides in 450 ms.
const Duration actionDelay = Duration(milliseconds: 60);
const Duration actionFadeDuration = Duration(milliseconds: 250);
const Duration actionBarDuration = Duration(milliseconds: 450);

/// The mode tooltips (SCREENSHOT-3): a box 18 pt high, ~10 pt of padding each side of the
/// text, four 1 pt corner ticks ~3 pt long, its bottom 32 pt above the bar's top edge.
const double tipHeight = 18;
const double tipPadding = 10;
const double tipTickLength = 3;
const double tipAboveBar = 32;

/// The Options menu (SCREENSHOT-3): 210 pt wide, opening upwards with its left edge 1 pt left
/// of the Options button and its bottom 22 pt below the bar's top edge, so it overlaps the
/// bar and leaves the "Options" label visible.
const double optionsMenuWidth = 210;
const double optionsMenuLeft = -1;
const double optionsMenuOverlap = 22;

/// When there is no room above the bar the menu opens below it with this gap (ours, not
/// measured).
const double optionsBelowGap = 6;

/// The action button's fill: `#0066d8` raw, sRGB `#006fdc` (K-05).
const Color actionBlue = Color(0xFF006FDC);

/// The bar colors (over the glass). Dark measured over grey `#7a7b80` (K-06): glyphs and
/// "Options" `#c7c8cd`, separators and the pill of the chosen mode `#8f9095` (white ~15 %),
/// chosen glyph `#dbdde2`, pressed pill −5 levels and glyph `#c3c4c9`, a 1 pt dark rim at the
/// top `#595a5e` (black ~28 %). Light: measured only under the desktop veil (K-05); pill and
/// separators derived by removing the veil (out = 0.514 · C + 0.486 · background:
/// background ~`#ddd8dd`, pill ~`#b4afb4`, black ~19 %), glyphs almost black; the pressed
/// pill in light is *estimated*.
@immutable
class ShotBarColors {
  const ShotBarColors({
    required this.glyph,
    required this.closeFill,
    required this.closeGlyph,
    required this.line,
    required this.chosen,
    required this.chosenGlyph,
    required this.chosenPressed,
    required this.chosenPressedGlyph,
    required this.topLine,
    required this.tipText,
    required this.tipTick,
    required this.tipFill,
  });

  final Color glyph;
  final Color closeFill;
  final Color closeGlyph;
  final Color line;
  final Color chosen;
  final Color chosenGlyph;
  final Color chosenPressed;
  final Color chosenPressedGlyph;

  /// The rim at the top of the bar (null: none).
  final Color? topLine;

  /// The tooltip of a mode button: light-gray text (lum ~164 over `#7b7b7b`), corner ticks
  /// (lum 99 over 123, black ~20 %) and a fill +1 level over the background (dark,
  /// SCREENSHOT-3). Light is estimated.
  final Color tipText;
  final Color tipTick;
  final Color tipFill;

  static const ShotBarColors light = ShotBarColors(
    glyph: Color(0xFF1D1D1F),
    closeFill: Color(0x33000000),
    closeGlyph: Color(0xFF3C3C3E),
    line: Color(0x30000000),
    chosen: Color(0x30000000),
    chosenGlyph: Color(0xFF1D1D1F),
    chosenPressed: Color(0x3A000000),
    chosenPressedGlyph: Color(0xFF3C3C3E),
    topLine: null,
    tipText: Color(0xFF3C3C3E),
    tipTick: Color(0x33FFFFFF),
    tipFill: Color(0x05000000),
  );

  static const ShotBarColors dark = ShotBarColors(
    glyph: Color(0xFFC7C8CD),
    closeFill: Color(0x33FFFFFF),
    closeGlyph: Color(0xFFC7C8CD),
    line: Color(0x26FFFFFF),
    chosen: Color(0x26FFFFFF),
    chosenGlyph: Color(0xFFDBDDE2),
    chosenPressed: Color(0x1EFFFFFF),
    chosenPressedGlyph: Color(0xFFC3C4C9),
    topLine: Color(0x47000000),
    tipText: Color(0xB3FFFFFF),
    tipTick: Color(0x33000000),
    tipFill: Color(0x05FFFFFF),
  );

  static ShotBarColors of(BuildContext context) =>
      MediaQuery.maybePlatformBrightnessOf(context) == Brightness.dark
      ? dark
      : light;
}

/// The tooltip of the six mode buttons of the bar (macOS texts, localized): close, Options
/// and the action button have none (SCREENSHOT-3).
@immutable
class ShotBarTips {
  const ShotBarTips({required this.capture, required this.record});

  /// Screen, window, selection.
  final List<String> capture;
  final List<String> record;
}

/// The tooltip of a mode button (SCREENSHOT-3): no bubble, a transparent box [tipHeight]
/// high marked by four corner ticks, width = the text + [tipPadding] each side.
class ShotTip extends StatelessWidget {
  const ShotTip({super.key, required this.text, required this.colors});

  final String text;
  final ShotBarColors colors;

  @override
  Widget build(BuildContext context) {
    return CustomPaint(
      key: const ValueKey('bar tooltip box'),
      painter: _TipBoxPainter(colors.tipFill, colors.tipTick),
      child: SizedBox(
        height: tipHeight,
        child: Row(
          mainAxisSize: MainAxisSize.min,
          crossAxisAlignment: CrossAxisAlignment.center,
          children: [
            const SizedBox(width: tipPadding),
            Text(
              text,
              maxLines: 1,
              softWrap: false,
              style: uiText(size: 11, color: colors.tipText),
            ),
            const SizedBox(width: tipPadding),
          ],
        ),
      ),
    );
  }
}

class _TipBoxPainter extends CustomPainter {
  const _TipBoxPainter(this.fill, this.tick);
  final Color fill;
  final Color tick;

  @override
  void paint(Canvas canvas, Size size) {
    canvas.drawRect(Offset.zero & size, Paint()..color = fill);
    final paint = Paint()
      ..color = tick
      ..style = PaintingStyle.stroke
      ..strokeWidth = 1;
    const t = tipTickLength;
    final r = (Offset.zero & size).deflate(0.5);
    final path = Path()
      ..moveTo(r.left, r.top + t)
      ..lineTo(r.left, r.top)
      ..lineTo(r.left + t, r.top)
      ..moveTo(r.right - t, r.top)
      ..lineTo(r.right, r.top)
      ..lineTo(r.right, r.top + t)
      ..moveTo(r.right, r.bottom - t)
      ..lineTo(r.right, r.bottom)
      ..lineTo(r.right - t, r.bottom)
      ..moveTo(r.left + t, r.bottom)
      ..lineTo(r.left, r.bottom)
      ..lineTo(r.left, r.bottom - t);
    canvas.drawPath(path, paint);
  }

  @override
  bool shouldRepaint(_TipBoxPainter old) =>
      old.fill != fill || old.tick != tick;
}

/// The bar itself; its place is decided by whoever hosts it. Its width follows
/// [showAction]: `toolbarSize` with the action button, 85 pt less on the right without it.
class ShotToolbar extends StatefulWidget {
  const ShotToolbar({
    super.key,
    required this.choice,
    required this.colors,
    required this.optionsLabel,
    required this.actionLabel,
    required this.tips,
    required this.showAction,
    required this.onChoose,
    required this.onClose,
    required this.onOptions,
    required this.onAction,
    required this.onMove,
    required this.onMoved,
  });

  final ToolbarChoice choice;
  final ShotBarColors colors;
  final String optionsLabel;
  final String actionLabel;
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

  @override
  State<ShotToolbar> createState() => _ShotToolbarState();
}

class _ShotToolbarState extends State<ShotToolbar> {
  /// The mode button whose tooltip is shown: 0..2 capture, 3..5 record. It stays after the
  /// pointer leaves, until another mode button is hovered or the bar goes.
  int? _tip;

  @override
  Widget build(BuildContext context) {
    final c = widget.colors;
    final tips = widget.tips;
    final choice = widget.choice;
    Widget at(double x, double width, Widget child) =>
        Positioned(left: x, top: 1, width: width, height: 50, child: child);
    // The six mode buttons: the pointer on one shows its tooltip at once.
    Widget tipped(int index, Widget child) =>
        MouseRegion(onEnter: (_) => setState(() => _tip = index), child: child);
    Widget mode(ToolbarChoice which, ShotGlyph glyph, String key) => at(
      captureX[which.index],
      modeWidth,
      tipped(
        which.index,
        ShotToolButton(
          key: ValueKey(key),
          chosen: choice == which,
          colors: c,
          onTap: () => widget.onChoose(which),
          child: (color) => CustomPaint(
            size: const Size(26, 20),
            painter: ShotModePainter(glyph, color),
          ),
        ),
      ),
    );
    Widget record(int index, ShotGlyph glyph) => at(
      recordX[index],
      modeWidth,
      // TODO(#153): screen recording; until then dimmed, no action.
      tipped(
        3 + index,
        Opacity(
          key: ValueKey('record ${glyph.name}'),
          opacity: 0.35,
          child: Center(
            child: CustomPaint(
              size: const Size(26, 20),
              painter: ShotModePainter(glyph, c.glyph, record: true),
            ),
          ),
        ),
      ),
    );
    final tip = _tip;
    final tipText = tip == null
        ? null
        : tip < 3
        ? tips.capture[tip]
        : tips.record[tip - 3];
    final tipX = tip == null
        ? 0.0
        : (tip < 3 ? captureX[tip] : recordX[tip - 3]) + modeWidth / 2;
    // The width: with the action button (shown) or without it, 450 ms after a 60 ms start.
    return TweenAnimationBuilder<double>(
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
              child: ProfiledGlass(
                profile: ShellGlassProfile.control,
                radius: 18,
                material: GlassMaterial.regular,
                child: Container(
                  key: const ValueKey('screenshot bar'),
                  decoration: BoxDecoration(
                    borderRadius: BorderRadius.circular(18),
                    border: c.topLine == null
                        ? null
                        : Border(top: BorderSide(color: c.topLine!)),
                  ),
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
                      at(
                        closeX,
                        18,
                        GestureDetector(
                          key: const ValueKey('close bar'),
                          onTap: widget.onClose,
                          child: Center(
                            child: Container(
                              width: 18,
                              height: 18,
                              decoration: BoxDecoration(
                                color: c.closeFill,
                                shape: BoxShape.circle,
                              ),
                              child: CustomPaint(
                                key: const ValueKey('close glyph'),
                                painter: CrossGlyph(
                                  c.closeGlyph,
                                  arm: 3.2,
                                  stroke: 1.5,
                                ),
                              ),
                            ),
                          ),
                        ),
                      ),
                      mode(
                        ToolbarChoice.screen,
                        ShotGlyph.screen,
                        'mode screen',
                      ),
                      mode(
                        ToolbarChoice.window,
                        ShotGlyph.window,
                        'mode window',
                      ),
                      mode(
                        ToolbarChoice.selection,
                        ShotGlyph.selection,
                        'mode selection',
                      ),
                      Positioned(
                        left: separator1X,
                        top: 12,
                        width: 1,
                        height: 28,
                        child: ColoredBox(color: c.line),
                      ),
                      record(0, ShotGlyph.screen),
                      record(1, ShotGlyph.window),
                      record(2, ShotGlyph.selection),
                      Positioned(
                        left: separator2X,
                        top: 12,
                        width: 1,
                        height: 28,
                        child: ColoredBox(color: c.line),
                      ),
                      at(
                        optionsX,
                        optionsWidth,
                        ShotToolButton(
                          key: const ValueKey('options'),
                          width: optionsWidth,
                          colors: c,
                          onTap: widget.onOptions,
                          child: (color) => Text(
                            widget.optionsLabel,
                            style: uiText(size: 12, color: color),
                          ),
                        ),
                      ),
                      _action(),
                    ],
                  ),
                ),
              ),
            ),
            if (tipText != null)
              // Centred on the button, its bottom `tipAboveBar` over the bar's top edge.
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
                      colors: c,
                    ),
                  ),
                ),
              ),
          ],
        ),
      ),
    );
  }

  /// The action button: 80 x 36, `actionInset` from the bar's top, bottom and right edges;
  /// it fades in 250 ms (after the same 60 ms start) as the bar's width changes.
  Widget _action() {
    return Positioned(
      left: actionX,
      top: actionInset,
      width: actionWidth,
      height: actionHeight,
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
              child: GestureDetector(
                key: const ValueKey('capture'),
                onTap: widget.onAction,
                child: Container(
                  key: const ValueKey('capture fill'),
                  width: actionWidth,
                  height: actionHeight,
                  decoration: BoxDecoration(
                    color: actionBlue,
                    borderRadius: BorderRadius.circular(actionRadius),
                  ),
                  alignment: Alignment.center,
                  child: Text(
                    widget.actionLabel,
                    style: uiText(
                      size: 13,
                      color: const Color(0xFFFFFFFF),
                    ).copyWith(fontWeight: FontWeight.w600),
                  ),
                ),
              ),
            ),
          );
        },
      ),
    );
  }
}

/// A bar button: the pill of the chosen mode (darker when pressed, K-06) and the glyph in
/// the color of its state. The pointer over a mode that is not chosen changes nothing.
class ShotToolButton extends StatefulWidget {
  const ShotToolButton({
    super.key,
    required this.child,
    required this.onTap,
    required this.colors,
    this.chosen = false,
    this.width = modeWidth,
  });

  final Widget Function(Color color) child;
  final VoidCallback onTap;
  final ShotBarColors colors;
  final bool chosen;
  final double width;

  @override
  State<ShotToolButton> createState() => _ShotToolButtonState();
}

class _ShotToolButtonState extends State<ShotToolButton> {
  bool _pressed = false;

  @override
  Widget build(BuildContext context) {
    final c = widget.colors;
    final pressedChosen = widget.chosen && _pressed;
    return GestureDetector(
      behavior: HitTestBehavior.opaque,
      onTapDown: (_) => setState(() => _pressed = true),
      onTapCancel: () => setState(() => _pressed = false),
      onTapUp: (_) => setState(() => _pressed = false),
      onTap: widget.onTap,
      child: Center(
        child: Container(
          width: widget.width,
          height: 40,
          decoration: BoxDecoration(
            color: !widget.chosen
                ? null
                : pressedChosen
                ? c.chosenPressed
                : c.chosen,
            borderRadius: BorderRadius.circular(10),
          ),
          alignment: Alignment.center,
          child: widget.child(
            !widget.chosen
                ? c.glyph
                : pressedChosen
                ? c.chosenPressedGlyph
                : c.chosenGlyph,
          ),
        ),
      ),
    );
  }
}

enum ShotGlyph { screen, window, selection }

/// The mode icons: the full screen, a window in a screen, a dashed selection; the record
/// ones with a dot.
class ShotModePainter extends CustomPainter {
  const ShotModePainter(this.glyph, this.color, {this.record = false});
  final ShotGlyph glyph;
  final Color color;
  final bool record;

  @override
  void paint(Canvas canvas, Size size) {
    final stroke = Paint()
      ..color = color
      ..style = PaintingStyle.stroke
      ..strokeWidth = 1.5;
    final outer = RRect.fromRectAndRadius(
      (Offset.zero & size).deflate(1),
      const Radius.circular(3),
    );
    switch (glyph) {
      case ShotGlyph.screen:
        canvas.drawRRect(outer, stroke);
        if (!record) canvas.drawRRect(outer.deflate(3), Paint()..color = color);
      case ShotGlyph.window:
        canvas.drawRRect(outer, stroke);
        final inner = RRect.fromRectAndRadius(
          Rect.fromLTWH(6, 5, size.width - 12, size.height - 10),
          const Radius.circular(2),
        );
        canvas.drawRRect(inner, record ? stroke : (Paint()..color = color));
      case ShotGlyph.selection:
        const dash = 3.0;
        final rect = (Offset.zero & size).deflate(2);
        final path = Path()..addRect(rect);
        for (final metric in path.computeMetrics()) {
          for (var d = 0.0; d < metric.length; d += 2 * dash) {
            canvas.drawPath(metric.extractPath(d, d + dash), stroke);
          }
        }
    }
    if (record) {
      canvas.drawCircle(
        size.center(Offset.zero),
        size.height * 0.2,
        Paint()..color = color,
      );
    }
  }

  @override
  bool shouldRepaint(ShotModePainter old) =>
      old.glyph != glyph || old.color != color || old.record != record;
}
