// A shell surface on Ossido's glass with the Amalfi drop shadow of its material
// (`materials.md`: Regular level 2, Thick level 3). Ossido draws the blur, tint and rims of
// the region ([ProfiledGlass]); the shadow stays the widget's own, painted outside the shape
// only, so it never shows through the translucent glass.

import 'package:flutter/widgets.dart';
import 'package:silica_ui/silica_ui.dart';

import 'glass_profile.dart';

/// [ProfiledGlass] of [profile] with smooth corners of [radius] and the drop shadow of
/// [material] in the appearance in effect.
class ShadowedGlass extends StatelessWidget {
  const ShadowedGlass({
    super.key,
    required this.profile,
    required this.radius,
    this.thick = false,
    this.shadow = true,
    this.group = 0,
    this.child,
  });

  final ShellGlassProfile profile;
  final double radius;

  /// Glass/Thick (shadow level 3) instead of Glass/Regular (level 2).
  final bool thick;

  /// Whether to draw the drop shadow.
  final bool shadow;

  /// The glass group: Ossido merges glass only within a group, and a higher group sits on top
  /// of a lower one (see `Glass.group`).
  final int group;
  final Widget? child;

  @override
  Widget build(BuildContext context) {
    final shadows = AmalfiTheme.of(context).shadows;
    final glass = ProfiledGlass(
      profile: profile,
      radius: radius,
      material: thick ? GlassMaterial.thick : GlassMaterial.regular,
      group: group,
      child: child,
    );
    if (!shadow) return glass;
    return DecoratedBox(
      decoration: AmalfiShapeDecoration(
        shape: AmalfiSmoothBorder(radius: radius),
        shadows: thick ? shadows.level3 : shadows.level2,
      ),
      child: glass,
    );
  }
}
