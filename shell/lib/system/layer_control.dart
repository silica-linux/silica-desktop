// The embedder's silica/layer channel: keyboard and input region of the layer, for the
// components that keep running while hidden (a closed Faro must take neither pointer
// nor keyboard).

import 'package:flutter/services.dart';

/// What a press or a scroll outside an open menu or panel does after closing it (#149).
enum TransientOutside {
  /// It closes the menu and nothing else.
  consume,

  /// It closes the panel and also reaches what is below.
  passThrough,
}

/// The policies of the shell's transients. PROVISIONAL until the reference's INPUT-01
/// measurement: menus consume the press outside, panels let it through; the hover of the
/// windows below always works.
abstract final class TransientPolicy {
  static const ({TransientOutside press, TransientOutside scroll}) menu = (
    press: TransientOutside.consume,
    scroll: TransientOutside.consume,
  );
  static const ({TransientOutside press, TransientOutside scroll}) panel = (
    press: TransientOutside.passThrough,
    scroll: TransientOutside.passThrough,
  );
}

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

  /// A menu or panel of this surface is open (silica-shell-v1 version 6, #149): with the
  /// input region as small as the menu, the windows below keep the pointer; Ossido sends
  /// `transientDismissed` (ShellChannel) when a press outside ends it. `rects` count as
  /// inside even under another surface. False when Ossido does not support it: then the
  /// caller covers the screen to see the clicks outside.
  static Future<bool> transient({
    List<Rect>? rects,
    ({TransientOutside press, TransientOutside scroll}) policy =
        TransientPolicy.menu,
  }) async {
    String name(TransientOutside outside) =>
        outside == TransientOutside.passThrough ? 'pass_through' : 'consume';
    try {
      return await channel.invokeMethod<bool>('setTransient', {
            'rects': rects == null
                ? null
                : [
                    for (final r in rects) [r.left, r.top, r.width, r.height],
                  ],
            'press': name(policy.press),
            'scroll': name(policy.scroll),
          }) ??
          false;
    } on MissingPluginException {
      return false;
    }
  }

  /// The menu or panel was closed by the shell itself (no `transientDismissed` follows).
  static Future<void> clearTransient() => _call('clearTransient', {});

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
