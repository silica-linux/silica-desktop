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
  LayerArbiter._(this.layer);

  /// The arbiter of `layer`: one per layer surface (several layers share an engine, #201).
  factory LayerArbiter.of(LayerControl layer) =>
      _arbiters.putIfAbsent(layer, () => LayerArbiter._(layer));

  static final Map<LayerControl, LayerArbiter> _arbiters = {};

  /// The arbiter of the process's only layer.
  static LayerArbiter get instance => LayerArbiter.of(LayerControl.main);

  final LayerControl layer;

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
    layer.inputRegion(all ? null : [for (final w in wishes) ...?w.rects]);
    layer.keyboard(wishes.any((w) => w.keyboard) ? 'exclusive' : 'none');
  }
}
