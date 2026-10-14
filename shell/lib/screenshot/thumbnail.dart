// The floating thumbnail after a capture (screenshot-ui.md, K-05): the image fitted in
// 256 × 144 inside a 4 pt white frame with corners of ~4 and a soft black shadow, 12 pt from
// the bottom right corner. What the pointer does on it:
// - a click opens the image;
// - a secondary click opens its menu;
// - a press that moves drags the file out (a Wayland drag and drop, the file written first);
// - a two-finger swipe to the right sends it away at once (PROVISIONAL: threshold);
// - while the pointer is on it, it does not leave (PROVISIONAL: not measured).

import 'dart:io';
import 'dart:math' as math;
import 'dart:ui' as ui;

import 'package:flutter/gestures.dart';
import 'package:flutter/widgets.dart';

const Size thumbnailImage = Size(256, 144);
const double thumbnailBorder = 4;
const double thumbnailMargin = 12;
const double thumbnailRadius = 4;

/// The fade-in when it appears (K-05: ~400 ms, slowing down).
const Duration thumbnailSettle = Duration(milliseconds: 400);

/// How long it stays (K-05: ~5.1 s).
const Duration thumbnailHold = Duration(milliseconds: 5100);

/// The slide off to the right (K-05: ~220 ms, accelerating).
const Duration thumbnailLeave = Duration(milliseconds: 220);

/// How far two fingers move to the right to send it away (PROVISIONAL).
const double thumbnailSwipe = 40;

/// The thumbnail box for an image of `image` size on a `screen`: fitted in 256 × 144, plus
/// the frame, 12 pt from the right and bottom edges.
Rect thumbnailRect(Size image, Size screen) {
  if (screen.isEmpty || image.isEmpty) return Rect.zero;
  final fit = math.min(
    thumbnailImage.width / image.width,
    thumbnailImage.height / image.height,
  );
  final w = image.width * fit + 2 * thumbnailBorder;
  final h = image.height * fit + 2 * thumbnailBorder;
  return Rect.fromLTWH(
    screen.width - thumbnailMargin - w,
    screen.height - thumbnailMargin - h,
    w,
    h,
  );
}

/// The frame with the image; the gestures go to the callbacks.
class ThumbnailView extends StatefulWidget {
  const ThumbnailView({
    super.key,
    required this.file,
    required this.onOpen,
    required this.onMenu,
    required this.onDragOut,
    required this.onSwipe,
    required this.onHover,
  });

  final String file;
  final VoidCallback onOpen;

  /// A secondary click at this point (layer coordinates).
  final ValueChanged<Offset> onMenu;

  /// The press moved: drag the file out, grabbed at this point of the thumbnail.
  final ValueChanged<Offset> onDragOut;
  final VoidCallback onSwipe;

  /// The pointer is on it (true) or left it.
  final ValueChanged<bool> onHover;

  @override
  State<ThumbnailView> createState() => _ThumbnailViewState();
}

class _ThumbnailViewState extends State<ThumbnailView> {
  double _swiped = 0;
  bool _dragging = false;

  void _horizontal(double dx) {
    // Only to the right; going back cancels the swipe.
    _swiped = math.max(0, _swiped + dx);
    if (_swiped >= thumbnailSwipe) {
      _swiped = double.negativeInfinity;
      widget.onSwipe();
    }
  }

  @override
  Widget build(BuildContext context) => MouseRegion(
    onEnter: (_) => widget.onHover(true),
    onExit: (_) => widget.onHover(false),
    child: Listener(
      // Two fingers on a touchpad: pan-zoom events, or scroll events where the embedder
      // sends them as such (natural scrolling: the content goes left when the fingers go
      // right).
      onPointerPanZoomStart: (_) => _swiped = 0,
      onPointerPanZoomUpdate: (e) => _horizontal(e.panDelta.dx),
      onPointerSignal: (e) {
        if (e is PointerScrollEvent &&
            e.scrollDelta.dx.abs() > e.scrollDelta.dy.abs()) {
          _horizontal(-e.scrollDelta.dx);
        }
      },
      child: GestureDetector(
        dragStartBehavior: DragStartBehavior.down,
        // Two fingers on a touchpad swipe it away: they never start a drag.
        supportedDevices: const {
          PointerDeviceKind.mouse,
          PointerDeviceKind.touch,
          PointerDeviceKind.stylus,
          PointerDeviceKind.invertedStylus,
        },
        onTap: widget.onOpen,
        onSecondaryTapUp: (d) => widget.onMenu(d.globalPosition),
        onPanStart: (d) {
          if (_dragging) return;
          _dragging = true;
          widget.onDragOut(d.localPosition);
        },
        onPanEnd: (_) => _dragging = false,
        onPanCancel: () => _dragging = false,
        child: Container(
          key: const ValueKey('thumbnail frame'),
          padding: const EdgeInsets.all(thumbnailBorder),
          decoration: BoxDecoration(
            color: const Color(0xFFFFFFFF),
            borderRadius: BorderRadius.circular(thumbnailRadius),
            // −10 levels at the edge, −5 at ~12 pt, −2 at ~25: black, the same on three sides.
            boxShadow: const [
              BoxShadow(color: Color(0x2E000000), blurRadius: 26),
            ],
          ),
          child: ClipRRect(
            borderRadius: BorderRadius.circular(1),
            child: Image.file(
              File(widget.file),
              fit: BoxFit.fill,
              gaplessPlayback: true,
              errorBuilder: (_, _, _) => const SizedBox.expand(),
            ),
          ),
        ),
      ),
    ),
  );
}

/// The image of the drag: the capture at the thumbnail's size inside its white frame, as
/// the thumbnail looks (decoded from the file; null if it cannot be read).
Future<ui.Image?> thumbnailDragImage(String file, Size box) async {
  try {
    final bytes = await File(file).readAsBytes();
    final inner = Size(
      box.width - 2 * thumbnailBorder,
      box.height - 2 * thumbnailBorder,
    );
    final codec = await ui.instantiateImageCodec(
      bytes,
      targetWidth: math.max(1, inner.width.round()),
      targetHeight: math.max(1, inner.height.round()),
    );
    final frame = await codec.getNextFrame();
    final recorder = ui.PictureRecorder();
    final canvas = Canvas(recorder);
    final w = box.width.round(), h = box.height.round();
    canvas.drawRRect(
      RRect.fromRectAndRadius(
        Rect.fromLTWH(0, 0, w.toDouble(), h.toDouble()),
        const Radius.circular(thumbnailRadius),
      ),
      Paint()..color = const Color(0xFFFFFFFF),
    );
    canvas.drawImage(
      frame.image,
      const Offset(thumbnailBorder, thumbnailBorder),
      Paint(),
    );
    frame.image.dispose();
    codec.dispose();
    return await recorder.endRecording().toImage(w, h);
  } on Exception {
    return null;
  }
}
