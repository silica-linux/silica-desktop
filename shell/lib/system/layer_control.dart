// Il canale silica/layer dell'embedder: tastiera e regione d'input del layer, per i
// componenti che restano in esecuzione nascosti (Faro chiuso non deve prendere né
// puntatore né tastiera).

import 'package:flutter/services.dart';

class LayerControl {
  const LayerControl._();

  static const MethodChannel channel = MethodChannel(
    'silica/layer',
    JSONMethodCodec(),
  );

  /// `none`, `exclusive` o `on-demand`.
  static Future<void> keyboard(String mode) =>
      _call('setKeyboardInteractivity', {'mode': mode});

  /// La superficie riceve il puntatore solo in questi rettangoli; null: ovunque.
  static Future<void> inputRegion(List<Rect>? rects) =>
      _call('setInputRegion', {
        'rects': rects == null
            ? null
            : [
                for (final r in rects) [r.left, r.top, r.width, r.height],
              ],
      });

  /// Dimensione della superficie in punti; 0 segue gli ancoraggi (la larghezza della barra).
  static Future<void> size(double width, double height) =>
      _call('setSize', {'width': width, 'height': height});

  static Future<void> _call(String method, Map<String, Object?> args) async {
    try {
      await channel.invokeMethod<Object?>(method, args);
    } on MissingPluginException {
      // Fuori da Silica (test).
    }
  }
}
