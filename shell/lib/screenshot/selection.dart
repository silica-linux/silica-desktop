// Choosing a region: the ⇧⌘4 crosshair and its live selection, and the adjustable selection
// of the ⇧⌘5 bar.
//
// ⇧⌘4 (screenshot-ui.md): a thin black cross of ~20 pt with a translucent grey disc in the
// center and, at the bottom right, the point in points on two lines; while dragging, the size
// of the selection instead. With the button held: Shift keeps one side fixed (the one that
// moved least), Option grows the selection from its center, Space moves it.
// ⇧⌘5 selection (PROVISIONAL, not measured yet): the screen outside the selection is dimmed,
// the selection has a thin white border and eight round handles; dragging a handle resizes,
// dragging inside moves, dragging outside draws a new one.

import 'dart:math' as math;

import 'package:flutter/gestures.dart';
import 'package:flutter/widgets.dart';
import 'package:silica_ui/silica_ui.dart';

/// The dim outside the bar's selection (PROVISIONAL).
const Color selectionDim = Color(0x59000000);

/// The handles of the bar's selection: diameter (PROVISIONAL).
const double handleSide = 9;

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

/// Text next to the crosshair: the point, or the size of the selection.
class CrosshairLabel extends StatelessWidget {
  const CrosshairLabel({super.key, required this.text});
  final String text;

  @override
  Widget build(BuildContext context) => Text(
    text,
    key: const ValueKey('coordinates'),
    style: TextStyle(
      fontFamily: uiFontFamily,
      package: 'silica_ui',
      fontSize: 10,
      height: 1.15,
      color: const Color(0xFF000000),
      shadows: const [Shadow(color: Color(0xCCFFFFFF), blurRadius: 3)],
    ),
  );
}

/// The thin black cross with the translucent grey disc in the center.
class CrosshairPainter extends CustomPainter {
  const CrosshairPainter();

  @override
  void paint(Canvas canvas, Size size) {
    final c = size.center(Offset.zero);
    final line = Paint()
      ..color = const Color(0xFF000000)
      ..strokeWidth = 1;
    canvas
      ..drawLine(Offset(c.dx, 0), Offset(c.dx, size.height), line)
      ..drawLine(Offset(0, c.dy), Offset(size.width, c.dy), line)
      ..drawCircle(c, 3, Paint()..color = const Color(0x66808080));
  }

  @override
  bool shouldRepaint(CrosshairPainter old) => false;
}

/// The camera of window mode.
class CameraPainter extends CustomPainter {
  const CameraPainter();

  @override
  void paint(Canvas canvas, Size size) {
    final body = Paint()..color = const Color(0xFF3A3A3C);
    final w = size.width;
    final h = size.height;
    canvas
      ..drawRRect(
        RRect.fromRectAndRadius(
          Rect.fromLTWH(0, h * 0.2, w, h * 0.8),
          const Radius.circular(4),
        ),
        body,
      )
      ..drawRect(Rect.fromLTWH(w * 0.35, 0, w * 0.3, h * 0.25), body)
      ..drawCircle(
        Offset(w / 2, h * 0.6),
        h * 0.25,
        Paint()..color = const Color(0xFFE5E5EA),
      )
      ..drawCircle(Offset(w / 2, h * 0.6), h * 0.15, body);
  }

  @override
  bool shouldRepaint(CameraPainter old) => false;
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
          child: CustomPaint(size: screen, painter: _SelectionPainter(r)),
        ),
      );
    },
  );
}

class _SelectionPainter extends CustomPainter {
  const _SelectionPainter(this.rect);
  final Rect rect;

  @override
  void paint(Canvas canvas, Size size) {
    final dim = Paint()..color = selectionDim;
    final all = Offset.zero & size;
    canvas
      ..drawRect(Rect.fromLTRB(0, 0, all.right, rect.top), dim)
      ..drawRect(Rect.fromLTRB(0, rect.bottom, all.right, all.bottom), dim)
      ..drawRect(Rect.fromLTRB(0, rect.top, rect.left, rect.bottom), dim)
      ..drawRect(
        Rect.fromLTRB(rect.right, rect.top, all.right, rect.bottom),
        dim,
      )
      ..drawRect(
        rect,
        Paint()
          ..color = const Color(0xFFFFFFFF)
          ..style = PaintingStyle.stroke
          ..strokeWidth = 1,
      );
    for (final (at, _) in _SelectionEditorState._handles(rect)) {
      canvas
        ..drawCircle(
          at,
          handleSide / 2,
          Paint()..color = const Color(0xFFFFFFFF),
        )
        ..drawCircle(
          at,
          handleSide / 2,
          Paint()
            ..color = const Color(0x66000000)
            ..style = PaintingStyle.stroke
            ..strokeWidth = 0.5,
        );
    }
  }

  @override
  bool shouldRepaint(_SelectionPainter old) => old.rect != rect;
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
