// Shared by the tests of the surfaces that use silica_ui's Amalfi widgets on Ossido's glass:
// a compositor glass region is declared (with a profile) and Flutter paints no tint, rim or
// backdrop of its own (that is `AmalfiGlassBackend.flutter`).

import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:silica_shell/system/glass_profile.dart';
import 'package:silica_ui/silica_ui.dart';

/// [child] under the compositor backend, as every shell layer is (`ShellComponent`).
Widget compositorScope(Widget child) => AmalfiGlassBackendScope(
  backend: AmalfiGlassBackend.compositor,
  child: child,
);

/// The profiles of the glass regions on the screen, as Ossido gets them: `AmalfiGlass`'s
/// (`RenderGlass.profile`) and the shell's own (`RenderProfiledGlass.surface`).
List<String> glassProfiles(WidgetTester tester) => [
  for (final glass in tester.renderObjectList<RenderGlass>(
    find.byWidgetPredicate((w) => w is Glass),
  ))
    if (glass is RenderProfiledGlass)
      glass.surface.name
    else
      ?glass.profile?.name,
];

/// Expects a glass region of [profile] among those declared.
void expectGlassRegion(WidgetTester tester, String profile) {
  expect(
    glassProfiles(tester),
    contains(profile),
    reason: 'a compositor glass region of the "$profile" profile',
  );
}

/// Expects that no Flutter-side glass is painted: no `AmalfiGlass` face (tint, rims), no
/// backdrop filter.
void expectNoFlutterTint(WidgetTester tester) {
  final faces = tester
      .widgetList<CustomPaint>(find.byType(CustomPaint))
      .where((p) => p.painter.runtimeType.toString() == '_GlassFacePainter');
  expect(faces, isEmpty, reason: 'AmalfiGlass painted a Flutter face');
  expect(find.byType(BackdropFilter), findsNothing);
}
