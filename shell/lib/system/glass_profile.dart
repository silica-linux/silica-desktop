// The glass profile of the shell's surfaces (silica-glass-v1 version 7, silica-desktop#168):
// Ossido picks the blur, lens, edge, tint and shadow of each kind of surface. The pinned
// silica_ui has no `Glass.profile` yet (it comes with 0.29.0): until then [ProfiledGlass] adds
// the `profile` key to the regions its glass sends on the `silica/glass` channel.

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
  List<Map<String, Object>> regions() => [
    for (final region in super.regions()) {...region, 'profile': _profile.name},
  ];
}
