// Frame-time benchmark for the embedder's renderers (silica-desktop#203): a scene that
// changes every frame, drawn in one or more views, and the engine's own timings of every
// frame (`FrameTiming`: build on the UI thread, raster on the raster thread).
//
// Dart arguments:
// - `--windows=N`: N views (N > 1 needs the embedder's `--multi-window`): the main one and
//   N - 1 windows opened with `openWindow` on `silica/window`.
// - `--frames=N`: frames measured after a warm-up of 120 (default 600).
// - `--out=PATH`: where the summary goes (default: standard output).
//
// The summary is one line: `framebench frames=N build_p50=… build_p95=… raster_p50=…
// raster_p95=… raster_p99=… total_p50=… total_p95=… interval_p50=… interval_p95=…` in
// milliseconds, `interval` being the time between the starts of two frames' rasterization.
import 'dart:async';
import 'dart:io';
import 'dart:math' as math;
import 'dart:ui' show FlutterView, FrameTiming, FramePhase;

import 'package:flutter/scheduler.dart';
import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';

/// The `silica/window` channel of the embedder (several windows).
const MethodChannel windowChannel = MethodChannel(
  'silica/window',
  JSONMethodCodec(),
);

/// Frames dropped from the start: shader compilation, the first layout.
const int warmUp = 120;

class BenchOptions {
  const BenchOptions({this.windows = 1, this.frames = 600, this.out});

  final int windows;
  final int frames;
  final String? out;

  static BenchOptions parse(List<String> args) {
    var windows = 1;
    var frames = 600;
    String? out;
    for (final arg in args) {
      if (arg.startsWith('--windows=')) {
        windows = int.tryParse(arg.substring(10))?.clamp(1, 8) ?? 1;
      } else if (arg.startsWith('--frames=')) {
        frames = int.tryParse(arg.substring(9))?.clamp(10, 100000) ?? 600;
      } else if (arg.startsWith('--out=')) {
        out = arg.substring(6);
      }
    }
    return BenchOptions(windows: windows, frames: frames, out: out);
  }
}

/// The `p`-th percentile (0–100) of `values`, in milliseconds.
double percentile(List<Duration> values, double p) {
  if (values.isEmpty) return 0;
  final sorted = values.map((d) => d.inMicroseconds).toList()..sort();
  final rank = (p / 100 * (sorted.length - 1)).round();
  return sorted[rank.clamp(0, sorted.length - 1)] / 1000;
}

/// The summary line of `timings`.
String summary(List<FrameTiming> timings) {
  final build = [for (final t in timings) t.buildDuration];
  final raster = [for (final t in timings) t.rasterDuration];
  final total = [for (final t in timings) t.totalSpan];
  final intervals = <Duration>[];
  for (var i = 1; i < timings.length; i++) {
    final a = timings[i - 1].timestampInMicroseconds(FramePhase.rasterStart);
    final b = timings[i].timestampInMicroseconds(FramePhase.rasterStart);
    if (b > a) intervals.add(Duration(microseconds: b - a));
  }
  String f(double v) => v.toStringAsFixed(2);
  return 'framebench frames=${timings.length}'
      ' build_p50=${f(percentile(build, 50))}'
      ' build_p95=${f(percentile(build, 95))}'
      ' raster_p50=${f(percentile(raster, 50))}'
      ' raster_p95=${f(percentile(raster, 95))}'
      ' raster_p99=${f(percentile(raster, 99))}'
      ' total_p50=${f(percentile(total, 50))}'
      ' total_p95=${f(percentile(total, 95))}'
      ' interval_p50=${f(percentile(intervals, 50))}'
      ' interval_p95=${f(percentile(intervals, 95))}';
}

void main(List<String> args) {
  runApp(BenchApp(options: BenchOptions.parse(args)));
}

class BenchApp extends StatefulWidget {
  const BenchApp({super.key, required this.options});

  final BenchOptions options;

  @override
  State<BenchApp> createState() => _BenchAppState();
}

class _BenchAppState extends State<BenchApp> {
  final List<FlutterView> others = [];
  final List<FrameTiming> timings = [];
  int seen = 0;
  bool done = false;

  @override
  void initState() {
    super.initState();
    SchedulerBinding.instance.addTimingsCallback(_onTimings);
    _openWindows();
  }

  Future<void> _openWindows() async {
    for (var i = 1; i < widget.options.windows; i++) {
      try {
        final id = await windowChannel.invokeMethod<Object?>('openWindow', {
          'title': 'Bench $i',
          'width': 480,
          'height': 360,
        });
        final views = WidgetsBinding.instance.platformDispatcher.views;
        if (id is int && mounted) {
          setState(() {
            for (final view in views) {
              if (view.viewId == id) others.add(view);
            }
          });
        }
      } on MissingPluginException {
        return;
      }
    }
  }

  void _onTimings(List<FrameTiming> batch) {
    if (done) return;
    for (final timing in batch) {
      seen++;
      if (seen > warmUp) timings.add(timing);
    }
    if (timings.length >= widget.options.frames) {
      done = true;
      _report(summary(timings));
    }
  }

  void _report(String line) {
    final out = widget.options.out;
    if (out == null) {
      stdout.writeln(line);
      return;
    }
    try {
      File(out).writeAsStringSync('$line\n');
    } on FileSystemException catch (err) {
      stderr.writeln('framebench: $out: $err');
    }
  }

  @override
  void dispose() {
    SchedulerBinding.instance.removeTimingsCallback(_onTimings);
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return ViewAnchor(
      view: others.isEmpty
          ? null
          : ViewCollection(
              views: [
                for (final view in others)
                  View(
                    view: view,
                    child: BenchScene(seed: view.viewId),
                  ),
              ],
            ),
      child: const BenchScene(seed: 0),
    );
  }
}

/// A scene that changes every frame: a moving gradient, rounded cards with shadows and
/// clipped text, the kinds of drawing the shell does.
class BenchScene extends StatefulWidget {
  const BenchScene({super.key, required this.seed});

  final int seed;

  @override
  State<BenchScene> createState() => _BenchSceneState();
}

class _BenchSceneState extends State<BenchScene>
    with SingleTickerProviderStateMixin {
  late final Ticker ticker;
  Duration elapsed = Duration.zero;

  @override
  void initState() {
    super.initState();
    ticker = createTicker((now) => setState(() => elapsed = now))..start();
  }

  @override
  void dispose() {
    ticker.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final t = elapsed.inMicroseconds / 1e6 + widget.seed;
    return Directionality(
      textDirection: TextDirection.ltr,
      child: DecoratedBox(
        decoration: BoxDecoration(
          gradient: LinearGradient(
            begin: Alignment(math.cos(t), math.sin(t)),
            end: Alignment(-math.cos(t), -math.sin(t)),
            colors: const [Color(0xFF203A8F), Color(0xFF8F2063)],
          ),
        ),
        child: LayoutBuilder(
          builder: (context, constraints) {
            final width = constraints.maxWidth;
            final height = constraints.maxHeight;
            return Stack(
              children: [
                for (var i = 0; i < 24; i++)
                  Positioned(
                    left: (width - 160) * (0.5 + 0.45 * math.sin(t + i)),
                    top:
                        (height - 64) *
                        (0.5 + 0.45 * math.cos(t * 0.7 + i * 1.3)),
                    width: 160,
                    height: 64,
                    child: DecoratedBox(
                      decoration: BoxDecoration(
                        color: const Color(0xCCFFFFFF),
                        borderRadius: BorderRadius.circular(14),
                        boxShadow: const [
                          BoxShadow(
                            color: Color(0x44000000),
                            blurRadius: 12,
                            offset: Offset(0, 4),
                          ),
                        ],
                      ),
                      child: Center(
                        child: Text(
                          'Card $i · ${(t * 60).floor()}',
                          style: const TextStyle(
                            fontSize: 15,
                            color: Color(0xFF1C1C1E),
                          ),
                        ),
                      ),
                    ),
                  ),
              ],
            );
          },
        ),
      ),
    );
  }
}
