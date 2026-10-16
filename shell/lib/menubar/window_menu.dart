// The window arrangement popover (windows.md, long press on the green button;
// measured in silica-reference, BACKLOG-1 / W-08), on Amalfi's popover (Glass/Thick, radius
// 16, the arrow pointing up at the button): two rows of four cells of 54 × 44 holding 25 × 20
// icons (the pointer's highlight is 46 × 36) below their headings, then the full screen item.
// The items run Ossido's `window.tile_*` / `window.arrange_*` commands on the focused window
// (the one of the button). Esc closes it; moving the pointer away does not. The delay (hover
// ~813 ms, press ~628 ms) and the animation of the frames are Ossido's.

import 'package:flutter/physics.dart';
import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';
import 'package:silica_ui/silica_ui.dart';

import '../l10n/l10n.dart';

const double windowMenuWidth = 242;

/// The rows' height with one display (the "Move to <display>" section only exists with
/// several displays; Ossido drives one for now).
const double windowMenuHeight = 192;
const double windowMenuHeightWithDisplays = 229;

/// A cell of the icon rows: 54 × 44, the icon (25 × 20) centered in it.
const Size windowMenuCell = Size(54, 44);
const double windowMenuRow = 44;
const Size windowMenuIcon = Size(25, 20);
const double windowMenuPitch = 54;

/// The highlight under the pointer.
const Size windowMenuHover = Size(46, 36);

/// Left margin of the first cell: four cells and the margins fill the rows' width.
const double windowMenuMargin = (windowMenuWidth - 4 * windowMenuPitch) / 2;

/// The popover's width: the rows plus the padding of the list and of the rows.
const double windowPopoverWidth =
    windowMenuWidth +
    2 * 8 + // AmalfiPopoverMetrics.listPadding
    2 * AmalfiMenuMetrics.padding;

/// The parts of an icon: rectangles as fractions of the box; the first is the chosen window
/// (dark), the others the other windows (grey).
typedef TileGlyph = List<Rect>;

const Map<String, TileGlyph> tileGlyphs = {
  'window.tile_left': [Rect.fromLTWH(0, 0, 0.5, 1)],
  'window.tile_right': [Rect.fromLTWH(0.5, 0, 0.5, 1)],
  'window.tile_top': [Rect.fromLTWH(0, 0, 1, 0.5)],
  'window.tile_bottom': [Rect.fromLTWH(0, 0.5, 1, 0.5)],
  'window.tile_fill': [Rect.fromLTWH(0, 0, 1, 1)],
  'window.arrange_2': [
    Rect.fromLTWH(0, 0, 0.5, 1),
    Rect.fromLTWH(0.5, 0, 0.5, 1),
  ],
  'window.arrange_3': [
    Rect.fromLTWH(0, 0, 0.5, 1),
    Rect.fromLTWH(0.5, 0, 0.5, 0.5),
    Rect.fromLTWH(0.5, 0.5, 0.5, 0.5),
  ],
  'window.arrange_4': [
    Rect.fromLTWH(0, 0, 0.5, 0.5),
    Rect.fromLTWH(0.5, 0, 0.5, 0.5),
    Rect.fromLTWH(0, 0.5, 0.5, 0.5),
    Rect.fromLTWH(0.5, 0.5, 0.5, 0.5),
  ],
};

/// The popover. It fades in and scales from 0.96 about the arrow on `spring.panel`
/// (motion.md; the opacity reaches 1 within the first third) and fades out with
/// `fade.instant`. `execute` runs a command of Ossido's catalog; the popover closes after a
/// choice.
class WindowMenuPopover extends StatefulWidget {
  const WindowMenuPopover({
    super.key,
    required this.tip,
    required this.execute,
    this.texts,
    this.onDismissed,
    this.onRectChanged,
  });

  /// The point the arrow shows, in the surface.
  final Offset tip;
  final ValueChanged<String> execute;
  final AppLocalizations? texts;

  /// The popover is gone (a choice, Esc, a press outside), once it has faded out.
  final VoidCallback? onDismissed;

  /// The popover's rectangle in the surface, when it is placed.
  final ValueChanged<Rect>? onRectChanged;

  @override
  State<WindowMenuPopover> createState() => WindowMenuPopoverState();
}

class WindowMenuPopoverState extends State<WindowMenuPopover>
    with SingleTickerProviderStateMixin {
  late final AnimationController _controller = AnimationController(
    vsync: this,
    // Assumption: not measured (motion.md "Popovers (open/close in general)").
    duration: AmalfiFades.instant.duration,
  );
  final GlobalKey _bodyKey = GlobalKey();
  bool _closing = false;
  Rect? _reported;

  @override
  void initState() {
    super.initState();
    // Assumption: not measured (motion.md): popovers open with spring.panel.
    const s = AmalfiSprings.panel;
    _controller.animateWith(
      SpringSimulation(
        SpringDescription(mass: 1, stiffness: s.stiffness, damping: s.damping),
        0,
        1,
        0,
      ),
    );
  }

  @override
  void dispose() {
    _controller.dispose();
    super.dispose();
  }

  /// Fades the popover out, then tells the bar.
  void close() {
    if (_closing || !mounted) return;
    _closing = true;
    _controller
      ..stop()
      ..value = _controller.value.clamp(0.0, 1.0)
      ..reverse().whenCompleteOrCancel(() => widget.onDismissed?.call());
  }

  /// Whether a press at `p` (in the surface) is outside the popover.
  bool outside(Offset p) => !(rect?.inflate(1).contains(p) ?? false);

  /// The popover's rectangle, once laid out.
  Rect? get rect {
    final box = _bodyKey.currentContext?.findRenderObject();
    if (box is! RenderBox || !box.attached || !box.hasSize) return null;
    return box.localToGlobal(Offset.zero) & box.size;
  }

  void _report() {
    final r = rect;
    if (r != null && r != _reported) {
      _reported = r;
      widget.onRectChanged?.call(r);
    }
  }

  void _choose(String command) {
    close();
    widget.execute(command);
  }

  @override
  Widget build(BuildContext context) {
    final texts = widget.texts ?? englishTexts;
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (mounted) _report();
    });
    final screen = MediaQuery.maybeSizeOf(context)?.width ?? double.infinity;
    final arrow = _PopoverLayout.arrowOffset(
      widget.tip,
      windowPopoverWidth,
      screen,
    );
    return CustomSingleChildLayout(
      delegate: _PopoverLayout(widget.tip),
      child: Focus(
        autofocus: true,
        onKeyEvent: (node, event) {
          if (event is KeyDownEvent &&
              event.logicalKey == LogicalKeyboardKey.escape) {
            close();
            return KeyEventResult.handled;
          }
          return KeyEventResult.ignored;
        },
        child: AnimatedBuilder(
          animation: _controller,
          builder: (context, child) {
            final t = _controller.value;
            final opacity = _closing ? t : t * 3;
            return Opacity(
              opacity: opacity.clamp(0.0, 1.0),
              child: Transform.scale(
                scale: _closing ? 1 : 0.96 + 0.04 * t,
                alignment: Alignment(arrow * 2 / windowPopoverWidth - 1, -1),
                child: IgnorePointer(ignoring: _closing, child: child),
              ),
            );
          },
          child: KeyedSubtree(
            key: _bodyKey,
            child: Stack(
              children: [
                AmalfiPopover(
                  arrowEdge: AmalfiPopoverArrowEdge.top,
                  arrowOffset: arrow,
                  padding: AmalfiPopoverMetrics.listPadding,
                  child: Padding(
                    padding: const EdgeInsets.all(AmalfiMenuMetrics.padding),
                    child: SizedBox(
                      width: windowMenuWidth,
                      child: Column(
                        mainAxisSize: MainAxisSize.min,
                        crossAxisAlignment: CrossAxisAlignment.stretch,
                        children: [
                          AmalfiMenuHeader(label: texts.halfScreen),
                          _row(const [
                            'window.tile_left',
                            'window.tile_right',
                            'window.tile_top',
                            'window.tile_bottom',
                          ]),
                          const AmalfiMenuSeparator(),
                          AmalfiMenuHeader(label: texts.arrange),
                          _row(const [
                            'window.tile_fill',
                            'window.arrange_2',
                            'window.arrange_3',
                            'window.arrange_4',
                          ]),
                          const AmalfiMenuSeparator(),
                          _FullScreenItem(
                            label: texts.fullScreen,
                            onPressed: () => _choose('window.fullscreen'),
                          ),
                        ],
                      ),
                    ),
                  ),
                ),
              ],
            ),
          ),
        ),
      ),
    );
  }

  Widget _row(List<String> commands) => SizedBox(
    height: windowMenuRow,
    child: Stack(
      children: [
        for (var i = 0; i < commands.length; i++)
          Positioned(
            left: windowMenuMargin + windowMenuPitch * i,
            width: windowMenuPitch,
            top: 0,
            bottom: 0,
            child: _TileButton(
              key: ValueKey(commands[i]),
              glyph: tileGlyphs[commands[i]] ?? const [],
              onPressed: () => _choose(commands[i]),
            ),
          ),
      ],
    ),
  );
}

/// Puts the popover below the arrow's tip, inside the surface.
class _PopoverLayout extends SingleChildLayoutDelegate {
  const _PopoverLayout(this.tip);

  final Offset tip;

  static const double margin = 8;

  /// The arrow's centre from the popover's left edge, with the popover kept inside `screen`.
  static double arrowOffset(Offset tip, double width, double screen) =>
      tip.dx - _left(tip, width, screen);

  static double _left(Offset tip, double width, double screen) {
    final most = screen - width - margin;
    return (tip.dx - width / 2).clamp(margin, most < margin ? margin : most);
  }

  @override
  BoxConstraints getConstraintsForChild(BoxConstraints constraints) =>
      constraints.loosen();

  @override
  Offset getPositionForChild(Size size, Size childSize) =>
      Offset(_left(tip, childSize.width, size.width), tip.dy);

  @override
  bool shouldRelayout(_PopoverLayout old) => old.tip != tip;
}

/// The full screen item: an Amalfi menu row, highlighted under the pointer.
class _FullScreenItem extends StatefulWidget {
  const _FullScreenItem({required this.label, required this.onPressed});

  final String label;
  final VoidCallback onPressed;

  @override
  State<_FullScreenItem> createState() => _FullScreenItemState();
}

class _FullScreenItemState extends State<_FullScreenItem> {
  bool _hover = false;

  @override
  Widget build(BuildContext context) => MouseRegion(
    onEnter: (_) => setState(() => _hover = true),
    onExit: (_) => setState(() => _hover = false),
    child: GestureDetector(
      behavior: HitTestBehavior.opaque,
      onTap: widget.onPressed,
      child: AmalfiMenuItem(
        label: widget.label,
        highlighted: _hover,
        icon: Builder(
          builder: (context) => SizedBox(
            width: 14,
            height: 11,
            child: CustomPaint(
              painter: TilePainter(
                const [Rect.fromLTWH(0, 0, 1, 1)],
                IconTheme.of(context).color ?? const Color(0xFF000000),
                stroke: 1.2,
              ),
            ),
          ),
        ),
      ),
    ),
  );
}

/// An icon of the row: in the text colour, with the soft accent under the pointer.
class _TileButton extends StatefulWidget {
  const _TileButton({super.key, required this.glyph, required this.onPressed});
  final TileGlyph glyph;
  final VoidCallback onPressed;

  @override
  State<_TileButton> createState() => _TileButtonState();
}

class _TileButtonState extends State<_TileButton> {
  bool _hover = false;

  @override
  Widget build(BuildContext context) {
    final colors = AmalfiTheme.of(context).colors;
    return MouseRegion(
      onEnter: (_) => setState(() => _hover = true),
      onExit: (_) => setState(() => _hover = false),
      child: GestureDetector(
        behavior: HitTestBehavior.opaque,
        onTap: widget.onPressed,
        child: Align(
          alignment: Alignment.center,
          child: Container(
            width: windowMenuHover.width,
            height: windowMenuHover.height,
            decoration: BoxDecoration(
              color: _hover ? colors.accentAmalfiSoft : null,
              borderRadius: BorderRadius.circular(
                AmalfiMenuMetrics.highlightRadius,
              ),
            ),
            alignment: Alignment.center,
            child: SizedBox.fromSize(
              size: windowMenuIcon,
              child: CustomPaint(
                painter: TilePainter(
                  widget.glyph,
                  _hover ? colors.accentAmalfi : colors.textPrimary,
                ),
              ),
            ),
          ),
        ),
      ),
    );
  }
}

/// The screen outline (radius 4, stroke 1.5) with the filled parts: the chosen window in
/// the text color, the others in grey.
class TilePainter extends CustomPainter {
  const TilePainter(this.glyph, this.color, {this.stroke = 1.5});
  final TileGlyph glyph;
  final Color color;
  final double stroke;

  @override
  void paint(Canvas canvas, Size size) {
    final outer = RRect.fromRectAndRadius(
      (Offset.zero & size).deflate(stroke / 2),
      Radius.circular(size.height * 0.2),
    );
    canvas.drawRRect(
      outer,
      Paint()
        ..color = color
        ..style = PaintingStyle.stroke
        ..strokeWidth = stroke,
    );
    final inner = (Offset.zero & size).deflate(stroke + 1);
    canvas.save();
    canvas.clipRRect(
      RRect.fromRectAndRadius(inner, Radius.circular(size.height * 0.12)),
    );
    for (var i = 0; i < glyph.length; i++) {
      final part = glyph[i];
      final rect = Rect.fromLTWH(
        inner.left + part.left * inner.width,
        inner.top + part.top * inner.height,
        part.width * inner.width,
        part.height * inner.height,
      ).deflate(glyph.length > 1 ? 0.75 : 0);
      canvas.drawRect(
        rect,
        Paint()..color = i == 0 ? color : color.withValues(alpha: 0.35),
      );
    }
    canvas.restore();
  }

  @override
  bool shouldRepaint(TilePainter old) =>
      old.glyph != glyph || old.color != color || old.stroke != stroke;
}
