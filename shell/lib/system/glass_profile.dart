// The glass profile of the shell's surfaces (silica-glass-v1 version 7, silica-desktop#168):
// Ossido picks the blur, lens, edge, tint and shadow of each kind of surface. The pinned
// silica_ui has no `Glass.profile` yet (it comes with 0.29.0): until then [ProfiledGlass] adds
// the `profile` key to the regions its glass sends on the `silica/glass` channel.
//
// Turned glass (silica-glass-v1 version 8): a glass under a rotation (the fan's labels, turned
// with their icon) also sends its `frame`, the transform of its own rectangle, so Ossido draws
// the turned shape instead of the upright bounding box. Also not in silica_ui yet.

import 'package:flutter/widgets.dart';
import 'package:silica_ui/silica_ui.dart';

/// The kinds of surface (the names of the protocol's `profile` enum).
enum ShellGlassProfile {
  menu,
  panel,
  module,
  control,
  dock,
  tooltip,
  sheet,
  banner,
  popover,
}

/// A [Glass] of a kind of surface. Same layout and regions as [Glass] (and as a
/// non-interactive `LiquidGlass`, which is a plain [Glass]); the shadow stays the widget's own.
class ProfiledGlass extends Glass {
  const ProfiledGlass({
    super.key,
    required this.profile,
    super.radius,
    super.material,
    super.group,
    super.tip,
    super.child,
  });

  final ShellGlassProfile profile;

  @override
  RenderGlass createRenderObject(BuildContext context) => RenderProfiledGlass(
    profile: profile,
    radius: radius,
    material: material,
    group: group,
    tip: tip,
  );

  @override
  void updateRenderObject(BuildContext context, RenderGlass renderObject) {
    super.updateRenderObject(context, renderObject);
    if (renderObject is RenderProfiledGlass) renderObject.profile = profile;
  }
}

/// The render object of [ProfiledGlass]: its regions carry the profile.
class RenderProfiledGlass extends RenderGlass {
  RenderProfiledGlass({
    required ShellGlassProfile profile,
    required super.radius,
    required super.material,
    super.group,
    super.tip,
  }) : _profile = profile;

  ShellGlassProfile _profile;
  ShellGlassProfile get profile => _profile;
  set profile(ShellGlassProfile value) {
    if (value == _profile) return;
    _profile = value;
    GlassRegistry.instance.schedule();
  }

  @override
  List<Map<String, Object>> regions() {
    final regions = super.regions();
    final transform = getTransformTo(null);
    // The local rectangles of the body and of the tip, in the order of the regions.
    final local = [
      Offset.zero & size,
      if (tip case final tip?) tipRect(tip, size),
    ];
    return [
      for (final (i, region) in regions.indexed)
        {
          ...region,
          'profile': _profile.name,
          if (i < local.length)
            if (glassFrame(transform, local[i], i == 0 ? radius : 0)
                case final frame?)
              'frame': frame,
        },
    ];
  }
}

/// The local rectangle of a glass's tip (as `RenderGlass.regions` lays it out).
Rect tipRect(GlassTip tip, Size size) => switch (tip.edge) {
  TipEdge.bottom => Rect.fromLTWH(
    tip.x - tip.width / 2,
    size.height,
    tip.width,
    tip.height,
  ),
  TipEdge.top => Rect.fromLTWH(
    tip.x - tip.width / 2,
    -tip.height,
    tip.width,
    tip.height,
  ),
  TipEdge.left => Rect.fromLTWH(
    -tip.height,
    tip.x - tip.width / 2,
    tip.height,
    tip.width,
  ),
  TipEdge.right => Rect.fromLTWH(
    size.width,
    tip.x - tip.width / 2,
    tip.height,
    tip.width,
  ),
};

/// The `frame` of a glass region whose transform to the surface turns it (silica-glass-v1
/// version 8, docs/protocols.md): `transform` [a, b, c, d, tx, ty] takes `local`'s own
/// rectangle (0, 0)–(width, height) to the surface (x' = a·x + c·y + tx, y' = b·x + d·y + ty),
/// with its size and radius before the transform. The embedder derives the centre, size and
/// angle from it. Null for an upright transform (translated or scaled): the region's
/// bounding box is already its shape.
Map<String, Object>? glassFrame(Matrix4 transform, Rect local, double radius) {
  final m = transform.storage;
  final (a, b, c, d) = (m[0], m[1], m[4], m[5]);
  if (b.abs() < 1e-9 && c.abs() < 1e-9) return null;
  return {
    'transform': [
      a,
      b,
      c,
      d,
      m[12] + a * local.left + c * local.top,
      m[13] + b * local.left + d * local.top,
    ],
    'width': local.width,
    'height': local.height,
    'radius': radius,
  };
}
