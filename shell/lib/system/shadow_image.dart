// Shadows drawn once into an image.
//
// Impeller (the Vulkan renderer of Silica OS) cannot pass an inherited opacity to a blurred
// draw under a clip ("Contents::SetInheritedOpacity should never be called when
// Contents::CanAcceptOpacity returns false") and draws it at full strength while an Opacity
// ancestor fades: the shadow stays on screen after the content. A texture accepts the
// opacity. The shadow is therefore rasterized once per layout and drawn as an image, which
// also spares the blur at every frame of a fade.

import 'dart:ui' as ui;

import 'package:flutter/widgets.dart';

/// Draws [paintShadow] through an image cached while [cacheKey], the size and the device
/// pixel ratio stay the same. [paintShadow] draws in logical pixels in the box of the
/// widget; what it draws up to [bleed] outside the box is kept. An ancestor Opacity (or
/// [opacity]) fades the image.
class CachedShadow extends StatefulWidget {
  const CachedShadow({
    super.key,
    required this.cacheKey,
    required this.paintShadow,
    this.bleed = 0,
    this.opacity = 1,
    this.hittable = true,
    this.child,
  });

  /// Equal for two shadows that look the same.
  final Object cacheKey;
  final void Function(Canvas canvas, Size size) paintShadow;
  final double bleed;
  final double opacity;

  /// False for a decoration that must not swallow the clicks of what is behind it.
  final bool hittable;
  final Widget? child;

  @override
  State<CachedShadow> createState() => CachedShadowState();
}

class CachedShadowState extends State<CachedShadow> {
  ui.Image? _image;
  Object? _key;

  /// Whether the shadow was rasterized into a cached image (for tests).
  bool get hasImage => _image != null;

  ui.Image _imageFor(Size size, double ratio) {
    final key = Object.hash(widget.cacheKey, size, ratio, widget.bleed);
    final cached = _image;
    if (cached != null && _key == key) return cached;
    final bleed = widget.bleed;
    final recorder = ui.PictureRecorder();
    final canvas = Canvas(recorder)
      ..scale(ratio)
      ..translate(bleed, bleed);
    widget.paintShadow(canvas, size);
    final picture = recorder.endRecording();
    final image = picture.toImageSync(
      ((size.width + 2 * bleed) * ratio).ceil().clamp(1, 16384),
      ((size.height + 2 * bleed) * ratio).ceil().clamp(1, 16384),
    );
    picture.dispose();
    cached?.dispose();
    _key = key;
    return _image = image;
  }

  @override
  void dispose() {
    _image?.dispose();
    _image = null;
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final ratio = MediaQuery.maybeDevicePixelRatioOf(context) ?? 1.0;
    return CustomPaint(
      painter: _ShadowImagePainter(
        this,
        widget.cacheKey,
        ratio,
        widget.opacity,
        widget.hittable,
      ),
      child: widget.child,
    );
  }
}

class _ShadowImagePainter extends CustomPainter {
  const _ShadowImagePainter(
    this.state,
    this.cacheKey,
    this.ratio,
    this.opacity,
    this.hittable,
  );

  final CachedShadowState state;
  final Object cacheKey;
  final double ratio;
  final double opacity;
  final bool hittable;

  @override
  void paint(Canvas canvas, Size size) {
    if (opacity <= 0 || size.isEmpty) return;
    final image = state._imageFor(size, ratio);
    final bleed = state.widget.bleed;
    canvas.drawImageRect(
      image,
      Rect.fromLTWH(0, 0, image.width.toDouble(), image.height.toDouble()),
      Rect.fromLTWH(-bleed, -bleed, image.width / ratio, image.height / ratio),
      Paint()
        ..color = Color.fromRGBO(0, 0, 0, opacity.clamp(0.0, 1.0))
        ..filterQuality = FilterQuality.low,
    );
  }

  @override
  bool? hitTest(Offset position) => hittable ? null : false;

  @override
  bool shouldRepaint(_ShadowImagePainter old) =>
      old.cacheKey != cacheKey || old.ratio != ratio || old.opacity != opacity;
}
