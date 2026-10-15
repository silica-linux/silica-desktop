import 'package:flutter_test/flutter_test.dart';
import 'package:silica_framebench/main.dart';

void main() {
  test('options are parsed and bounded', () {
    final options = BenchOptions.parse([
      '--windows=3',
      '--frames=50',
      '--out=/dev/shm/bench.txt',
    ]);
    expect(options.windows, 3);
    expect(options.frames, 50);
    expect(options.out, '/dev/shm/bench.txt');
    final bounded = BenchOptions.parse(['--windows=99', '--frames=1', 'x']);
    expect(bounded.windows, 8);
    expect(bounded.frames, 10);
    final garbage = BenchOptions.parse(['--windows=many']);
    expect(garbage.windows, 1);
    expect(garbage.frames, 600);
  });

  test('percentiles are in milliseconds', () {
    final values = [
      for (var i = 1; i <= 100; i++) Duration(microseconds: i * 100),
    ];
    expect(percentile(values, 50), closeTo(5.0, 0.11));
    expect(percentile(values, 95), closeTo(9.5, 0.11));
    expect(percentile(const [], 50), 0);
  });

  testWidgets('the scene animates and builds in one view', (tester) async {
    await tester.pumpWidget(const BenchApp(options: BenchOptions()));
    await tester.pump(const Duration(milliseconds: 16));
    expect(find.textContaining('Card 0'), findsOneWidget);
    expect(find.textContaining('Card 23'), findsOneWidget);
  });
}
