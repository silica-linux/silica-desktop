// Vetro calcolato da Ossido (silica-glass-v1, docs/protocols.md): il widget dichiara la
// sua area, il compositor ci mette dietro lo sfondo sfocato del materiale. Il figlio
// disegna sopra (testo, icone) e lascia trasparente il resto.

import 'package:flutter/foundation.dart';
import 'package:flutter/rendering.dart';
import 'package:flutter/scheduler.dart';
import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';

/// Le due famiglie di vetro misurate: chiaro (Dock) e spesso (menu, ricerca).
enum GlassMaterial { clear, thick }

/// Un'area di vetro grande quanto il figlio, con angoli ad arco di cerchio.
class Glass extends SingleChildRenderObjectWidget {
  const Glass({
    super.key,
    this.radius = 0,
    this.material = GlassMaterial.clear,
    super.child,
  });

  final double radius;
  final GlassMaterial material;

  @override
  RenderGlass createRenderObject(BuildContext context) =>
      RenderGlass(radius: radius, material: material);

  @override
  void updateRenderObject(BuildContext context, RenderGlass renderObject) {
    renderObject
      ..radius = radius
      ..material = material;
  }
}

/// Il render object di [Glass]: si registra finché è attaccato all'albero.
class RenderGlass extends RenderProxyBox {
  RenderGlass({required double radius, required GlassMaterial material})
      : _radius = radius,
        _material = material;

  double _radius;
  double get radius => _radius;
  set radius(double value) {
    if (value == _radius) return;
    _radius = value;
    GlassRegistry.instance.schedule();
  }

  GlassMaterial _material;
  GlassMaterial get material => _material;
  set material(GlassMaterial value) {
    if (value == _material) return;
    _material = value;
    GlassRegistry.instance.schedule();
  }

  @override
  void attach(PipelineOwner owner) {
    super.attach(owner);
    GlassRegistry.instance.add(this);
  }

  @override
  void detach() {
    GlassRegistry.instance.remove(this);
    super.detach();
  }

  @override
  void paint(PaintingContext context, Offset offset) {
    // Dopo ogni disegno la posizione può essere cambiata.
    GlassRegistry.instance.schedule();
    super.paint(context, offset);
  }

  /// La regione in pixel logici della superficie.
  Map<String, Object> region() {
    final origin = localToGlobal(Offset.zero);
    return {
      'x': origin.dx,
      'y': origin.dy,
      'width': size.width,
      'height': size.height,
      'radius': radius,
      'material': material.name,
    };
  }
}

/// Raccoglie le aree di vetro dopo ogni frame e le manda all'embedder quando cambiano.
class GlassRegistry {
  GlassRegistry._();

  static final GlassRegistry instance = GlassRegistry._();

  static const MethodChannel channel = MethodChannel('silica/glass', JSONMethodCodec());

  final Set<RenderGlass> _glasses = {};
  List<Map<String, Object>> _sent = const [];
  bool _scheduled = false;

  void add(RenderGlass glass) {
    _glasses.add(glass);
    schedule();
  }

  void remove(RenderGlass glass) {
    _glasses.remove(glass);
    schedule();
  }

  /// Le regioni attuali, nell'ordine di registrazione.
  @visibleForTesting
  List<Map<String, Object>> get regions => [
        for (final glass in _glasses)
          if (glass.attached && glass.hasSize) glass.region(),
      ];

  void schedule() {
    if (_scheduled) return;
    _scheduled = true;
    SchedulerBinding.instance.addPostFrameCallback((_) {
      _scheduled = false;
      _send();
    });
    SchedulerBinding.instance.ensureVisualUpdate();
  }

  Future<void> _send() async {
    final regions = this.regions;
    if (listEquals(regions.map(_key).toList(), _sent.map(_key).toList())) return;
    _sent = regions;
    try {
      await channel.invokeMethod<Object?>('setRegions', regions);
    } on MissingPluginException {
      // Fuori da Silica (test, altri embedder) il vetro non c'è.
    }
  }

  static String _key(Map<String, Object> region) => region.entries
      .map((entry) => '${entry.key}=${entry.value}')
      .join(',');
}
