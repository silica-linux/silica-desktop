// Several widgets in the same layer (the app switcher, the screenshots, force quit): each
// says which part of the surface it wants and whether it wants the keyboard; the arbiter
// merges the requests and talks to the embedder once. The whole surface wins over rectangles.

import 'dart:ui';

import 'layer_control.dart';

/// What a widget asks for: nothing, some rectangles or the whole surface (null).
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
