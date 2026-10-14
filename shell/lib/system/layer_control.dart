// The embedder's silica/layer channel: keyboard and input region of the layer, for the
// components that keep running while hidden (a closed Faro must take neither pointer
// nor keyboard).

import 'package:flutter/services.dart';

class LayerControl {
  const LayerControl._();

  static const MethodChannel channel = MethodChannel(
    'silica/layer',
    JSONMethodCodec(),
  );

  /// `none`, `exclusive` or `on-demand`.
  static Future<void> keyboard(String mode) =>
      _call('setKeyboardInteractivity', {'mode': mode});

  /// The surface receives the pointer only in these rectangles; null: everywhere.
  static Future<void> inputRegion(List<Rect>? rects) =>
      _call('setInputRegion', {
        'rects': rects == null
            ? null
            : [
                for (final r in rects) [r.left, r.top, r.width, r.height],
              ],
      });

  /// Surface size in points; 0 follows the anchors (the width of the bar).
  static Future<void> size(double width, double height) =>
      _call('setSize', {'width': width, 'height': height});

  /// The screen size in points (null outside Silica or when unknown).
  static Future<Size?> outputSize() async {
    try {
      final size = await channel.invokeMethod<Object?>('outputSize');
      if (size is Map && size['width'] is num && size['height'] is num) {
        return Size(
          (size['width'] as num).toDouble(),
          (size['height'] as num).toDouble(),
        );
      }
    } on MissingPluginException {
      // Outside Silica.
    }
    return null;
  }

  static Future<void> _call(String method, Map<String, Object?> args) async {
    try {
      await channel.invokeMethod<Object?>(method, args);
    } on MissingPluginException {
      // Outside Silica (tests).
    }
  }
}
