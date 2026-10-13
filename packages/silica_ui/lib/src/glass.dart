// Vetro calcolato da Ossido (silica-glass-v1, docs/protocols.md): il widget dichiara la
// sua area, il compositor ci mette dietro lo sfondo sfocato del materiale. Il figlio
// disegna sopra (testo, icone) e lascia trasparente il resto.

import 'package:flutter/foundation.dart';
import 'package:flutter/rendering.dart';
import 'package:flutter/scheduler.dart';
import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';

/// I vetri misurati: chiaro (Dock), spesso (menu, ricerca), regolare (il Liquid Glass
/// dei controlli, con la lente forte sul bordo) e notifica (banner e Centro notifiche).
enum GlassMaterial { clear, thick, regular, notification }

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

  /// La regione in pixel logici della superficie, con le trasformazioni degli antenati
  /// (un vetro che si gonfia cresce anche nel compositor, raggio compreso).
  Map<String, Object> region() {
    final transform = getTransformTo(null);
    final rect = MatrixUtils.transformRect(transform, Offset.zero & size);
    final factor = size.width > 0 ? rect.width / size.width : 1.0;
    return {
      'x': rect.left,
      'y': rect.top,
      'width': rect.width,
      'height': rect.height,
      'radius': radius * factor,
      'material': material.name,
    };
  }
}

/// Raccoglie le aree di vetro dopo ogni frame e le manda all'embedder quando cambiano.
///
/// Si controlla a ogni frame, non solo quando un [RenderGlass] si ridisegna: un antenato
/// che cambia trasformazione o opacità (un pannello che si apre) può spostare il vetro
/// senza ridisegnarlo, e il compositor resterebbe con la geometria vecchia.
class GlassRegistry {
  GlassRegistry._();

  static final GlassRegistry instance = GlassRegistry._();

  static const MethodChannel channel = MethodChannel(
    'silica/glass',
    JSONMethodCodec(),
  );

  final Set<RenderGlass> _glasses = {};
  List<Map<String, Object>> _sent = const [];
  bool _scheduled = false;
  bool _everyFrame = false;

  void add(RenderGlass glass) {
    _glasses.add(glass);
    if (!_everyFrame) {
      _everyFrame = true;
      // Dopo il disegno di ogni frame (i callback persistenti girano in ordine di
      // registrazione, dopo quello del rendering).
      SchedulerBinding.instance.addPersistentFrameCallback((_) {
        if (_glasses.isNotEmpty || _sent.isNotEmpty) _send();
      });
    }
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
    if (listEquals(regions.map(_key).toList(), _sent.map(_key).toList()))
      return;
    _sent = regions;
    try {
      await channel.invokeMethod<Object?>('setRegions', regions);
    } on MissingPluginException {
      // Fuori da Silica (test, altri embedder) il vetro non c'è.
    }
  }

  static String _key(Map<String, Object> region) =>
      region.entries.map((entry) => '${entry.key}=${entry.value}').join(',');
}
