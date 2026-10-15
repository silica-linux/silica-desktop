import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:silica_shell/system/glass_profile.dart';
import 'package:silica_ui/silica_ui.dart';

// The shell's glass declares its kind of surface (#168): the regions sent on silica/glass carry
// the profile, the body and the tip alike; changing it sends them again.
void main() {
  testWidgets('a profiled glass sends its profile with every region', (
    tester,
  ) async {
    final calls = <MethodCall>[];
    tester.binding.defaultBinaryMessenger.setMockMethodCallHandler(
      GlassRegistry.channel,
      (call) async {
        calls.add(call);
        return true;
      },
    );
    Widget glass(ShellGlassProfile profile) => Directionality(
      textDirection: TextDirection.ltr,
      child: Stack(
        children: [
          Positioned(
            left: 10,
            top: 20,
            width: 100,
            height: 24,
            child: ProfiledGlass(
              profile: profile,
              radius: 12,
              material: GlassMaterial.thick,
              tip: const GlassTip(x: 50, width: 31, height: 10),
              child: const SizedBox.expand(),
            ),
          ),
        ],
      ),
    );
    await tester.pumpWidget(glass(ShellGlassProfile.tooltip));
    await tester.pump();
    var regions = (calls.last.arguments as List<Object?>)
        .cast<Map<Object?, Object?>>();
    expect(regions, hasLength(2));
    expect(regions.map((r) => r['profile']), everyElement('tooltip'));
    expect(regions.first['material'], 'thick');
    expect(regions.first['width'], 100);

    await tester.pumpWidget(glass(ShellGlassProfile.control));
    await tester.pump();
    regions = (calls.last.arguments as List<Object?>)
        .cast<Map<Object?, Object?>>();
    expect(regions.map((r) => r['profile']), everyElement('control'));
  });
}
