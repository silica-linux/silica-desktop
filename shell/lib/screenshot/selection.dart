// Choosing a region: the ⇧⌘4 crosshair and its live selection, and the adjustable selection
// of the ⇧⌘5 bar (kit "App · Screenshot": `Selection`, `Window Select`).
//
// ⇧⌘4 (screenshot-ui.md): the crosshair (Pointers/Crosshair: four 2 × 9 arms with a 6 pt gap,
// `Text/Primary` with a 1.5 pt white outline) and, 20 pt right of it, a 48 × 32 chip with the
// point in points on two lines; while dragging, the rest of the screen is dimmed and a capsule
// with the size of the selection sits at its bottom right. With the button held: Shift keeps
// one side fixed (the one that moved least), Option grows the selection from its center, Space
// moves it.
// ⇧⌘5 selection: the screen outside the selection is dimmed (`Text/Primary` 30 % in light,
// `Surface/Sunken` 50 % in dark), the selection has a 2 pt `Accent/Amalfi` border inside and eight
// 8 × 8 handles (`Surface/Raised`, 1 pt accent border, radius 2); dragging a handle resizes,
// dragging inside moves, dragging outside draws a new one. The behaviour is the measured one; the
// default selection (half the screen, centred) is PROVISIONAL (#158).

import 'dart:math' as math;

import 'package:flutter/gestures.dart';
import 'package:flutter/widgets.dart';
import 'package:silica_ui/silica_ui.dart';

import '../system/glass_profile.dart';
import '../system/shadowed_glass.dart';

/// The dim over the screen outside the selection (kit `Selection`: `Dim *`).
Color selectionDim(AmalfiThemeData theme) => theme.isDark
    ? theme.colors.surfaceSunken.withValues(alpha: 0.5)
    : theme.colors.textPrimary.withValues(alpha: 0.3);

/// The handles of the bar's selection (kit `Handle 1`..`8`: 8 × 8, radius 2).
const double handleSide = 8;
const double handleRadius = 2;

/// The selection's border (kit `Selection`: 2 pt, inside).
const double selectionBorder = 2;

/// The tint of the target (the window under the pointer, or the screen): `Accent/Amalfi` at 50 %
/// (kit `Window tint`, `Screen tint`).
Color targetTint(AmalfiThemeData theme) =>
    theme.colors.accentAmalfi.withValues(alpha: 0.5);

/// The default selection of the bar when it does not remember one: half the screen, centered
/// (PROVISIONAL).
Rect defaultSelection(Size screen) => Rect.fromCenter(
  center: screen.center(Offset.zero),
  width: screen.width / 2,
  height: screen.height / 2,
);

/// The ⇧⌘4 selection while the button is held: where it started, the modifiers.
class LiveSelection {
  LiveSelection(this.start) : end = start;

  /// The corner where the drag started (moved by Space).
  Offset start;
  Offset end;

  /// Shift: the axis kept (null: none yet).
  Axis? locked;
  Offset? lockedAt;

  /// Space: the last pointer while moving.
  Offset? moving;

  /// The rectangle for the pointer at `p` with the modifiers held.
  Rect update(Offset p, {required bool shift, required bool option}) {
    if (moving case final last?) {
      final d = p - last;
      start += d;
      end += d;
      moving = p;
      return rect(option: option);
    }
    var point = p;
    if (shift) {
      lockedAt ??= end;
      final from = lockedAt!;
      if (locked == null) {
        final d = p - from;
        // The side that moves least stays fixed (after a couple of points).
        if (d.distance >= 2) {
          locked = d.dx.abs() >= d.dy.abs() ? Axis.vertical : Axis.horizontal;
        }
      }
      point = switch (locked) {
        Axis.vertical => Offset(p.dx, from.dy),
        Axis.horizontal => Offset(from.dx, p.dy),
        null => from,
      };
    } else {
      locked = null;
      lockedAt = null;
    }
    end = point;
    return rect(option: option);
  }

  /// The selection: between the start and the end, or centered on the start with Option.
  Rect rect({required bool option}) => option
      ? Rect.fromPoints(start - (end - start), end)
      : Rect.fromPoints(start, end);
}

/// The chip next to the crosshair: the pointer's x and y on two lines (kit `Coordinates
/// capsule`: 48 × 32, radius 8, Caption), on the compositor's Regular glass.
class CrosshairLabel extends StatelessWidget {
  const CrosshairLabel({super.key, required this.text});
  final String text;

  @override
  Widget build(BuildContext context) {
    final colors = AmalfiTheme.of(context).colors;
    return SizedBox(
      width: 48,
      height: 32,
      child: ShadowedGlass(
        profile: ShellGlassProfile.control,
        radius: 8,
        child: Center(
          child: Text(
            text,
            key: const ValueKey('coordinates'),
            textAlign: TextAlign.center,
            style: AmalfiType.caption(
              color: colors.textPrimary,
            ).copyWith(fontWeight: FontWeight.w500),
          ),
        ),
      ),
    );
  }
}

/// The capsule with the size of a selection (kit `Size capsule`: 96 × 24, radius 12, Label).
class SelectionSizeLabel extends StatelessWidget {
  const SelectionSizeLabel({super.key, required this.size});
  final Size size;

  @override
  Widget build(BuildContext context) {
    final colors = AmalfiTheme.of(context).colors;
    return ConstrainedBox(
      constraints: const BoxConstraints(minWidth: 96, minHeight: 24),
      child: IntrinsicWidth(
        child: ShadowedGlass(
          profile: ShellGlassProfile.control,
          radius: 12,
          child: SizedBox(
            height: 24,
            child: Padding(
              padding: const EdgeInsets.symmetric(horizontal: 12),
              child: Center(
                child: Text(
                  '${size.width.round()} × ${size.height.round()}',
                  key: const ValueKey('size label'),
                  maxLines: 1,
                  softWrap: false,
                  style: AmalfiType.label(color: colors.textPrimary),
                ),
              ),
            ),
          ),
        ),
      ),
    );
  }
}

/// Where the size capsule goes for a selection `rect` on a `screen`: 8 pt below its bottom edge,
/// right-aligned 8 pt inside the right edge (kit `Selection`); inside the selection when there is
/// no room below. `width` is the capsule's.
Offset selectionSizeAt(Rect rect, Size screen, double width) {
  final below = rect.bottom + 8;
  final y = below + 24 <= screen.height ? below : rect.bottom - 8 - 24;
  final x = (rect.right - 8 - width)
      .clamp(0.0, math.max(0.0, screen.width - width))
      .toDouble();
  return Offset(x, y);
}

/// The thin cross of the ⇧⌘4 crosshair (kit `Pointers/Crosshair`): four 2 × 9 arms around a 6 pt
/// gap in `Text/Primary` with a 1.5 pt white outline, in a 24 × 24 box.
class CrosshairPainter extends CustomPainter {
  const CrosshairPainter(this.ink);

  final Color ink;

  @override
  void paint(Canvas canvas, Size size) {
    final c = size.center(Offset.zero);
    const arm = 9.0, gap = 3.0, half = 1.0;
    final arms = [
      Rect.fromLTRB(c.dx - half, c.dy - gap - arm, c.dx + half, c.dy - gap),
      Rect.fromLTRB(c.dx - half, c.dy + gap, c.dx + half, c.dy + gap + arm),
      Rect.fromLTRB(c.dx - gap - arm, c.dy - half, c.dx - gap, c.dy + half),
      Rect.fromLTRB(c.dx + gap, c.dy - half, c.dx + gap + arm, c.dy + half),
    ];
    final outline = Paint()
      ..color = const Color(0xFFFFFFFF)
      ..style = PaintingStyle.stroke
      ..strokeWidth = 3
      ..strokeJoin = StrokeJoin.miter;
    final fill = Paint()..color = ink;
    for (final r in arms) {
      canvas.drawRect(r, outline);
    }
    for (final r in arms) {
      canvas.drawRect(r, fill);
    }
  }

  @override
  bool shouldRepaint(CrosshairPainter old) => old.ink != ink;
}

/// The camera of window mode (kit `Window Select`): a 32 pt Glass/Regular disc with the Icon Kit
/// camera at 16 under the pointer.
class ShotCamera extends StatelessWidget {
  const ShotCamera({super.key});

  @override
  Widget build(BuildContext context) {
    final colors = AmalfiTheme.of(context).colors;
    return SizedBox.square(
      dimension: 32,
      child: ShadowedGlass(
        profile: ShellGlassProfile.control,
        radius: 16,
        child: Center(
          child: AmalfiSymbol('camera', size: 16, color: colors.textPrimary),
        ),
      ),
    );
  }
}

/// What a press on the bar's selection grabs.
enum _Grab { none, move, draw, left, top, right, bottom, tl, tr, bl, br }

/// The adjustable selection of the ⇧⌘5 bar, filling the screen.
class SelectionEditor extends StatefulWidget {
  const SelectionEditor({
    super.key,
    required this.rect,
    required this.onChanged,
    required this.onCommitted,
  });

  final Rect rect;
  final ValueChanged<Rect> onChanged;

  /// A drag ended (the selection to remember).
  final ValueChanged<Rect> onCommitted;

  @override
  State<SelectionEditor> createState() => _SelectionEditorState();
}

class _SelectionEditorState extends State<SelectionEditor> {
  _Grab _grab = _Grab.none;
  Offset _from = Offset.zero;
  Rect _startRect = Rect.zero;
  MouseCursor _cursor = SystemMouseCursors.precise;
  Rect? _latest;

  static List<(Offset, _Grab)> _handles(Rect r) => [
    (r.topLeft, _Grab.tl),
    (r.topCenter, _Grab.top),
    (r.topRight, _Grab.tr),
    (r.centerRight, _Grab.right),
    (r.bottomRight, _Grab.br),
    (r.bottomCenter, _Grab.bottom),
    (r.bottomLeft, _Grab.bl),
    (r.centerLeft, _Grab.left),
  ];

  _Grab _grabAt(Offset p) {
    final r = widget.rect;
    for (final (at, grab) in _handles(r)) {
      if ((p - at).distance <= handleSide) return grab;
    }
    return r.contains(p) ? _Grab.move : _Grab.draw;
  }

  static MouseCursor _cursorFor(_Grab grab) => switch (grab) {
    _Grab.move => SystemMouseCursors.move,
    _Grab.left || _Grab.right => SystemMouseCursors.resizeLeftRight,
    _Grab.top || _Grab.bottom => SystemMouseCursors.resizeUpDown,
    _Grab.tl || _Grab.br => SystemMouseCursors.resizeUpLeftDownRight,
    _Grab.tr || _Grab.bl => SystemMouseCursors.resizeUpRightDownLeft,
    _Grab.none || _Grab.draw => SystemMouseCursors.precise,
  };

  Rect _resized(Offset p, Size screen) {
    final r = _startRect;
    final d = p - _from;
    final bounds = Offset.zero & screen;
    Rect clampMove(Rect m) {
      final dx = m.left < 0
          ? -m.left
          : m.right > screen.width
          ? screen.width - m.right
          : 0.0;
      final dy = m.top < 0
          ? -m.top
          : m.bottom > screen.height
          ? screen.height - m.bottom
          : 0.0;
      return m.shift(Offset(dx, dy));
    }

    final next = switch (_grab) {
      _Grab.move => clampMove(r.shift(d)),
      _Grab.draw => Rect.fromPoints(_from, p),
      _Grab.left => Rect.fromLTRB(r.left + d.dx, r.top, r.right, r.bottom),
      _Grab.right => Rect.fromLTRB(r.left, r.top, r.right + d.dx, r.bottom),
      _Grab.top => Rect.fromLTRB(r.left, r.top + d.dy, r.right, r.bottom),
      _Grab.bottom => Rect.fromLTRB(r.left, r.top, r.right, r.bottom + d.dy),
      _Grab.tl => Rect.fromLTRB(r.left + d.dx, r.top + d.dy, r.right, r.bottom),
      _Grab.tr => Rect.fromLTRB(r.left, r.top + d.dy, r.right + d.dx, r.bottom),
      _Grab.bl => Rect.fromLTRB(r.left + d.dx, r.top, r.right, r.bottom + d.dy),
      _Grab.br => Rect.fromLTRB(r.left, r.top, r.right + d.dx, r.bottom + d.dy),
      _Grab.none => r,
    };
    // Dragging a side past the other flips it, as a fresh drag would.
    final normal = Rect.fromPoints(next.topLeft, next.bottomRight);
    return normal.intersect(bounds);
  }

  /// The capsule with the size, 8 pt below the selection's bottom right corner.
  Widget _sizeLabel(Rect r, Size screen) {
    final at = selectionSizeAt(r, screen, 96);
    return Positioned(
      left: at.dx,
      top: at.dy,
      child: IgnorePointer(child: SelectionSizeLabel(size: r.size)),
    );
  }

  @override
  Widget build(BuildContext context) => LayoutBuilder(
    builder: (context, box) {
      final screen = box.biggest;
      final r = widget.rect;
      return MouseRegion(
        cursor: _cursor,
        onHover: (e) {
          final cursor = _cursorFor(_grabAt(e.localPosition));
          if (cursor != _cursor) setState(() => _cursor = cursor);
        },
        child: GestureDetector(
          dragStartBehavior: DragStartBehavior.down,
          key: const ValueKey('selection editor'),
          behavior: HitTestBehavior.opaque,
          onPanStart: (d) {
            _grab = _grabAt(d.localPosition);
            _from = d.localPosition;
            _startRect = r;
          },
          onPanUpdate: (d) {
            final next = _resized(d.localPosition, screen);
            _latest = next;
            widget.onChanged(next);
          },
          // The last update may not be drawn yet: it is the one to remember.
          onPanEnd: (_) {
            _grab = _Grab.none;
            widget.onCommitted(_latest ?? widget.rect);
            _latest = null;
          },
          child: Stack(
            children: [
              Positioned.fill(
                child: CustomPaint(
                  painter: _SelectionPainter(r, AmalfiTheme.of(context)),
                ),
              ),
              _sizeLabel(r, screen),
            ],
          ),
        ),
      );
    },
  );
}

class _SelectionPainter extends CustomPainter {
  const _SelectionPainter(this.rect, this.theme, {this.handles = true});
  final Rect rect;
  final AmalfiThemeData theme;
  final bool handles;

  @override
  void paint(Canvas canvas, Size size) {
    final dim = Paint()..color = selectionDim(theme);
    final all = Offset.zero & size;
    canvas
      ..drawRect(Rect.fromLTRB(0, 0, all.right, rect.top), dim)
      ..drawRect(Rect.fromLTRB(0, rect.bottom, all.right, all.bottom), dim)
      ..drawRect(Rect.fromLTRB(0, rect.top, rect.left, rect.bottom), dim)
      ..drawRect(
        Rect.fromLTRB(rect.right, rect.top, all.right, rect.bottom),
        dim,
      );
    paintSelectionBorder(canvas, rect, theme);
    if (!handles) return;
    for (final (at, _) in _SelectionEditorState._handles(rect)) {
      final box = Rect.fromCenter(
        center: at,
        width: handleSide,
        height: handleSide,
      );
      final shape = RRect.fromRectAndRadius(
        box,
        const Radius.circular(handleRadius),
      );
      canvas
        ..drawRRect(shape, Paint()..color = theme.colors.surfaceRaised)
        ..drawRRect(
          shape.deflate(0.5),
          Paint()
            ..color = theme.colors.accentAmalfi
            ..style = PaintingStyle.stroke
            ..strokeWidth = 1,
        );
    }
  }

  @override
  bool shouldRepaint(_SelectionPainter old) =>
      old.rect != rect || old.theme != theme || old.handles != handles;
}

/// The selection of ⇧⌘4 while the button is held: the screen outside it dimmed, its 2 pt accent
/// border, and the capsule with its size (kit `Selection`). It fills the layer.
class SelectionFrame extends StatelessWidget {
  const SelectionFrame({super.key, required this.rect});

  final Rect rect;

  @override
  Widget build(BuildContext context) => LayoutBuilder(
    builder: (context, box) {
      final screen = box.biggest;
      final at = selectionSizeAt(rect, screen, 96);
      return Stack(
        children: [
          Positioned.fill(
            child: IgnorePointer(
              child: CustomPaint(
                painter: _SelectionPainter(
                  rect,
                  AmalfiTheme.of(context),
                  handles: false,
                ),
              ),
            ),
          ),
          Positioned.fromRect(
            rect: rect,
            child: const SizedBox.expand(key: ValueKey('selection')),
          ),
          Positioned(
            left: at.dx,
            top: at.dy,
            child: IgnorePointer(child: SelectionSizeLabel(size: rect.size)),
          ),
        ],
      );
    },
  );
}

/// The selection's 2 pt `Accent/Amalfi` border, inside `rect`.
void paintSelectionBorder(Canvas canvas, Rect rect, AmalfiThemeData theme) {
  canvas.drawRect(
    rect.deflate(selectionBorder / 2),
    Paint()
      ..color = theme.colors.accentAmalfi
      ..style = PaintingStyle.stroke
      ..strokeWidth = selectionBorder,
  );
}

/// A rectangle clamped inside the screen, at least 1 × 1 (a remembered selection from a
/// larger screen).
Rect fitSelection(Rect r, Size screen) {
  final bounds = Offset.zero & screen;
  final fitted = r.intersect(bounds);
  if (fitted.width < 1 || fitted.height < 1) return defaultSelection(screen);
  return Rect.fromLTWH(
    fitted.left,
    fitted.top,
    math.max(1, fitted.width),
    math.max(1, fitted.height),
  );
}
