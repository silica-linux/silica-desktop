// The floating thumbnail after a capture (screenshot-ui.md, K-05, SCREENSHOT-2): the image
// fitted in 256 × 144 inside a 4 pt frame (white in light appearance; black with a 1 pt light
// line on its inner edge in dark) with corners of ~4 and a soft black shadow, 12 pt from the
// bottom right corner. What the pointer does on it:
// - a click opens the image;
// - a secondary click opens its menu;
// - a press that moves drags the image out (a Wayland drag and drop of a file that only
//   exists for the drag: the target gets the image, nothing stays where it was saved);
// - a two-finger swipe to the right sends it away and saves it at once (PROVISIONAL:
//   threshold);
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

/// The fade-in when it appears (SCREENSHOT-2: ~380 ms, slowing down).
const Duration thumbnailSettle = Duration(milliseconds: 380);

/// How long it stays (SCREENSHOT-2: ~5.4 s). Its menu pauses the time.
const Duration thumbnailHold = Duration(milliseconds: 5400);

/// The slide off to the right (SCREENSHOT-2: ~250 ms, accelerating).
const Duration thumbnailLeave = Duration(milliseconds: 250);

/// The long side of the image that follows the pointer in a drag (SCREENSHOT-2: 128 pt,
/// frameless, the pointer ~12 pt above and to the left of its center).
const double thumbnailDragSide = 128;
const double thumbnailDragPointer = 12;

/// The dark frame: black with a 1 pt line of this colour on its inner edge.
const Color thumbnailDarkFrame = Color(0xFF000000);
const Color thumbnailDarkLine = Color(0xFF949494);
const double thumbnailDarkLineWidth = 1;

/// The size of the drag image for a capture of `image` size: 128 pt on the long side.
Size thumbnailDragSize(Size image) {
  if (image.isEmpty) return Size.zero;
  final fit = thumbnailDragSide / math.max(image.width, image.height);
  return Size(
    math.max(1, (image.width * fit).roundToDouble()),
    math.max(1, (image.height * fit).roundToDouble()),
  );
}

/// Where the drag image is held: its center, moved so that the pointer is
/// `thumbnailDragPointer` above and to the left of it.
Offset thumbnailDragHotspot(Size drag) => Offset(
  drag.width / 2 - thumbnailDragPointer,
  drag.height / 2 - thumbnailDragPointer,
);

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

  /// The press moved: drag the image out.
  final VoidCallback onDragOut;
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
          widget.onDragOut();
        },
        onPanEnd: (_) => _dragging = false,
        onPanCancel: () => _dragging = false,
        child: _frame(
          context,
          ClipRRect(
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

  /// The frame around the image: white in light appearance; in dark black, with a 1 pt light
  /// line on its inner edge and black behind the transparent parts of the image.
  Widget _frame(BuildContext context, Widget image) {
    final dark =
        MediaQuery.maybePlatformBrightnessOf(context) == Brightness.dark;
    // −10 levels at the edge, −5 at ~12 pt, −2 at ~25: black, the same on three sides.
    const shadow = [BoxShadow(color: Color(0x2E000000), blurRadius: 26)];
    final radius = BorderRadius.circular(thumbnailRadius);
    return Container(
      key: const ValueKey('thumbnail frame'),
      padding: EdgeInsets.all(
        dark ? thumbnailBorder - thumbnailDarkLineWidth : thumbnailBorder,
      ),
      decoration: BoxDecoration(
        color: dark ? thumbnailDarkFrame : const Color(0xFFFFFFFF),
        borderRadius: radius,
        boxShadow: shadow,
      ),
      child: dark
          ? DecoratedBox(
              key: const ValueKey('thumbnail line'),
              decoration: const BoxDecoration(
                border: Border.fromBorderSide(
                  BorderSide(
                    color: thumbnailDarkLine,
                    width: thumbnailDarkLineWidth,
                  ),
                ),
              ),
              child: ColoredBox(color: thumbnailDarkFrame, child: image),
            )
          : image,
    );
  }
}

/// The image of the drag: the capture alone, with no frame, 128 pt on its long side
/// (decoded from the file; null if it cannot be read).
Future<ui.Image?> thumbnailDragImage(String file, Size image) async {
  try {
    final bytes = await File(file).readAsBytes();
    final size = thumbnailDragSize(image);
    if (size.isEmpty) return null;
    final codec = await ui.instantiateImageCodec(
      bytes,
      targetWidth: size.width.round(),
      targetHeight: size.height.round(),
    );
    final frame = await codec.getNextFrame();
    codec.dispose();
    return frame.image;
  } on Exception {
    return null;
  }
}
