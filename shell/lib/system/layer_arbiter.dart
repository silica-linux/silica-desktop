// Più widget nello stesso layer (il selettore delle app, le istantanee, l'uscita forzata):
// ognuno dice quale parte della superficie vuole e se vuole la tastiera, l'arbitro unisce le
// richieste e parla con l'embedder una volta. Tutta la superficie vince sui rettangoli.

import 'dart:ui';

import 'layer_control.dart';

/// Quello che un widget chiede: niente, dei rettangoli o tutta la superficie (null).
class LayerWish {
  const LayerWish.none() : rects = const [], keyboard = false;
  const LayerWish.rects(this.rects) : keyboard = false;
  const LayerWish.all({this.keyboard = true}) : rects = null;

  final List<Rect>? rects;
  final bool keyboard;
}

class LayerArbiter {
  LayerArbiter._();

  static final LayerArbiter instance = LayerArbiter._();

  final Map<Object, LayerWish> _wishes = {};

  void set(Object owner, LayerWish wish) {
    _wishes[owner] = wish;
    _apply();
  }

  void remove(Object owner) {
    if (_wishes.remove(owner) != null) _apply();
  }

  void _apply() {
    final wishes = _wishes.values;
    final all = wishes.any((w) => w.rects == null);
    LayerControl.inputRegion(
      all ? null : [for (final w in wishes) ...?w.rects],
    );
    LayerControl.keyboard(wishes.any((w) => w.keyboard) ? 'exclusive' : 'none');
  }
}
