// The ⇧⌘5 bar (screenshot-ui.md, K-05/K-06): glass ~545 × 52, 4 pt above the Dock area,
// where it was last left. Left to right: Close (a disc with ×), capture the screen, a window,
// a selection (44 × 50 at a 48 pt pitch), a separator, record the screen, a window, a
// selection (at a 57 pt pitch), a separator, Options with an arrow (76 × 50) and the blue
// action button (80 × 50). The chosen mode sits on a rounded pill; the pointer over a mode
// that is not chosen changes nothing. The bar is moved by dragging its background.
//
// Recording is not available yet (#153, PipeWire screencast): its three
// buttons are drawn dimmed and do nothing.

import 'package:flutter/gestures.dart';
import 'package:flutter/widgets.dart';
import 'package:silica_ui/silica_ui.dart';

import 'settings.dart';

const Size toolbarSize = Size(545, 52);

/// From the Dock area (84 pt exclusive zone) to the bar: 4 pt.
const double toolbarAboveDock = 84 + 4;

/// Where the controls sit, from the bar's left edge (AX frames, K-05). The separators, the
/// Options button and the action button are PROVISIONAL (placed from the measured widths).
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
  );

  static ShotBarColors of(BuildContext context) =>
      MediaQuery.maybePlatformBrightnessOf(context) == Brightness.dark
      ? dark
      : light;
}

/// The bar itself, `toolbarSize`; its place is decided by whoever hosts it.
class ShotToolbar extends StatelessWidget {
  const ShotToolbar({
    super.key,
    required this.choice,
    required this.colors,
    required this.optionsLabel,
    required this.actionLabel,
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

  /// The action button exists only while there is a target (K-05).
  final bool showAction;
  final ValueChanged<ToolbarChoice> onChoose;
  final VoidCallback onClose;
  final VoidCallback onOptions;
  final VoidCallback onAction;

  /// The background is dragged: the bar moves by this much.
  final ValueChanged<Offset> onMove;
  final VoidCallback onMoved;

  @override
  Widget build(BuildContext context) {
    final c = colors;
    Widget at(double x, double width, Widget child) =>
        Positioned(left: x, top: 1, width: width, height: 50, child: child);
    Widget mode(ToolbarChoice which, ShotGlyph glyph, String key) => at(
      captureX[which.index],
      modeWidth,
      ShotToolButton(
        key: ValueKey(key),
        chosen: choice == which,
        colors: c,
        onTap: () => onChoose(which),
        child: (color) => CustomPaint(
          size: const Size(26, 20),
          painter: ShotModePainter(glyph, color),
        ),
      ),
    );
    Widget record(int index, ShotGlyph glyph) => at(
      recordX[index],
      modeWidth,
      // TODO(#153): screen recording; until then dimmed, no action.
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
    );
    return SizedBox.fromSize(
      size: toolbarSize,
      child: LiquidGlass(
        radius: 18,
        material: GlassMaterial.regular,
        interactive: false,
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
                  onPanUpdate: (d) => onMove(d.delta),
                  onPanEnd: (_) => onMoved(),
                ),
              ),
              at(
                closeX,
                18,
                GestureDetector(
                  key: const ValueKey('close bar'),
                  onTap: onClose,
                  child: Center(
                    child: Container(
                      width: 18,
                      height: 18,
                      decoration: BoxDecoration(
                        color: c.closeFill,
                        shape: BoxShape.circle,
                      ),
                      alignment: Alignment.center,
                      child: Text(
                        '×',
                        style: uiText(size: 13, color: c.closeGlyph),
                      ),
                    ),
                  ),
                ),
              ),
              mode(ToolbarChoice.screen, ShotGlyph.screen, 'mode screen'),
              mode(ToolbarChoice.window, ShotGlyph.window, 'mode window'),
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
                  onTap: onOptions,
                  child: (color) =>
                      Text(optionsLabel, style: uiText(size: 12, color: color)),
                ),
              ),
              if (showAction)
                at(
                  actionX,
                  actionWidth,
                  GestureDetector(
                    key: const ValueKey('capture'),
                    onTap: onAction,
                    child: Center(
                      // The visible pill inside the 80 × 50 button: PROVISIONAL.
                      child: Container(
                        width: 76,
                        height: 30,
                        decoration: BoxDecoration(
                          color: actionBlue,
                          borderRadius: BorderRadius.circular(15),
                        ),
                        alignment: Alignment.center,
                        child: Text(
                          actionLabel,
                          style: uiText(
                            size: 13,
                            color: const Color(0xFFFFFFFF),
                          ).copyWith(fontWeight: FontWeight.w600),
                        ),
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
