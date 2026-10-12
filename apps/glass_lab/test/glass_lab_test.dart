import 'package:flutter_test/flutter_test.dart';
import 'package:silica_glass_lab/main.dart';
import 'package:silica_ui/silica_ui.dart';

void main() {
  testWidgets('le capsule si trascinano', (tester) async {
    await tester.pumpWidget(const GlassLab());
    expect(find.byType(LiquidGlass), findsNWidgets(5));
    final start = tester.getTopLeft(find.byType(LiquidGlass).first);
    await tester.drag(find.text('Rete'), const Offset(30, 20));
    await tester.pumpAndSettle();
    expect(tester.getTopLeft(find.byType(LiquidGlass).first), start + const Offset(30, 20));
  });
}
