// Test app for the Silica embedder: known colors, so that the tests read the
// screenshot.
//
// - Every tap toggles the background between [idle] and [tapped].
// - Typing "word" in the text field makes the background [typed] (text input).
// - Esc clears taps and text (key events, through HardwareKeyboard).
// - The square at the top left tells the devicePixelRatio: [hiDpi] from 2 up,
//   otherwise [loDpi].
// - At the top right a glass area ([glassSize], silica-glass-v1): the opaque background
//   covers it, it is for the tests that read `glass.list`.
// - F2 opens a second window ([secondTitle], [secondSize], silica-desktop#146) drawn in its
//   own view, all [second] ([typed] once its own field holds the magic word): a tap in it
//   closes it, and so does its close button (the app answers `closeRequested` and closes it
//   itself).
// - F3 exports the main window for another app (`exportHandle`, xdg-foreign) and shows the
//   identifier it got ("wayland:<handle>") as the window title, for the tests to read.
import 'dart:ui' show FlutterView;

import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';
import 'package:silica_ui/silica_ui.dart';

const Color idle = Color(0xFF3366CC);
const Color tapped = Color(0xFFCC6633);
const Color typed = Color(0xFF33CC66);
const Color loDpi = Color(0xFFFF00FF);
const Color hiDpi = Color(0xFF00FF00);
const double probeSize = 16;
const String magicWord = 'word';
const Size glassSize = Size(40, 20);
const Color second = Color(0xFF8833AA);
const String secondTitle = 'Second';
const Size secondSize = Size(200, 120);

/// The `silica/window` channel of the embedder (several windows).
const MethodChannel windowChannel = MethodChannel(
  'silica/window',
  JSONMethodCodec(),
);

void main() => runApp(const DemoApp());

class DemoApp extends StatefulWidget {
  const DemoApp({super.key});

  @override
  State<DemoApp> createState() => _DemoAppState();
}

class _DemoAppState extends State<DemoApp> {
  int taps = 0;
  final TextEditingController text = TextEditingController();
  final FocusNode textFocus = FocusNode();

  /// The second window's view, while it is open.
  FlutterView? secondView;

  @override
  void initState() {
    super.initState();
    text.addListener(() => setState(() {}));
    HardwareKeyboard.instance.addHandler(_onKey);
    windowChannel.setMethodCallHandler(_onWindowCall);
  }

  Future<Object?> _onWindowCall(MethodCall call) async {
    final args = call.arguments;
    final id = args is Map ? args['viewId'] : null;
    switch (call.method) {
      case 'closeRequested':
        // The app closes the window itself (it could ask to save first).
        await _closeSecond(id);
        return true;
      case 'windowClosed':
        if (id == secondView?.viewId) setState(() => secondView = null);
    }
    return null;
  }

  Future<void> _openSecond() async {
    if (secondView != null) return;
    final id = await windowChannel.invokeMethod<Object?>('openWindow', {
      'title': secondTitle,
      'width': secondSize.width,
      'height': secondSize.height,
      'minWidth': 150,
      'minHeight': 100,
      'parent': 0,
    });
    if (id is! int || !mounted) return;
    final views = WidgetsBinding.instance.platformDispatcher.views;
    setState(() {
      for (final view in views) {
        if (view.viewId == id) secondView = view;
      }
    });
  }

  Future<void> _exportWindow() async {
    final handle = await windowChannel.invokeMethod<Object?>('exportHandle');
    if (handle is! String) return;
    await windowChannel.invokeMethod<Object?>('setTitle', {'title': handle});
  }

  Future<void> _closeSecond(Object? id) async {
    if (id == null) return;
    await windowChannel.invokeMethod<Object?>('closeWindow', {'viewId': id});
  }

  @override
  void dispose() {
    HardwareKeyboard.instance.removeHandler(_onKey);
    text.dispose();
    textFocus.dispose();
    super.dispose();
  }

  bool _onKey(KeyEvent event) {
    if (event is KeyDownEvent &&
        event.logicalKey == LogicalKeyboardKey.escape) {
      setState(() => taps = 0);
      text.clear();
      return true;
    }
    if (event is KeyDownEvent && event.logicalKey == LogicalKeyboardKey.f2) {
      _openSecond();
      return true;
    }
    if (event is KeyDownEvent && event.logicalKey == LogicalKeyboardKey.f3) {
      _exportWindow();
      return true;
    }
    return false;
  }

  Color get background {
    if (text.text == magicWord) return typed;
    return taps.isEven ? idle : tapped;
  }

  @override
  Widget build(BuildContext context) {
    final view = secondView;
    return ViewAnchor(
      view: view == null
          ? null
          : View(
              view: view,
              child: SecondWindow(onTap: () => _closeSecond(view.viewId)),
            ),
      child: _main(context),
    );
  }

  Widget _main(BuildContext context) {
    const white = Color(0xFFFFFFFF);
    return Directionality(
      textDirection: TextDirection.ltr,
      child: GestureDetector(
        behavior: HitTestBehavior.opaque,
        onTap: () => setState(() => taps++),
        child: ColoredBox(
          color: background,
          child: Stack(
            children: [
              Positioned(
                left: 0,
                top: 0,
                width: probeSize,
                height: probeSize,
                child: ColoredBox(
                  color: MediaQuery.devicePixelRatioOf(context) >= 2
                      ? hiDpi
                      : loDpi,
                ),
              ),
              Positioned(
                right: 0,
                top: 0,
                width: glassSize.width,
                height: glassSize.height,
                child: const Glass(radius: 6, child: SizedBox.expand()),
              ),
              Center(
                child: Text(
                  'Silica · $taps',
                  style: const TextStyle(fontSize: 32, color: white),
                ),
              ),
              Positioned(
                left: 24,
                right: 24,
                bottom: 16,
                child: EditableText(
                  controller: text,
                  focusNode: textFocus,
                  autofocus: true,
                  onTapOutside: (_) {},
                  style: const TextStyle(fontSize: 20, color: white),
                  cursorColor: white,
                  backgroundCursorColor: const Color(0xFF000000),
                ),
              ),
            ],
          ),
        ),
      ),
    );
  }
}

/// The second window's content: all [second], [typed] once its own field holds the magic
/// word (the keyboard reaches the field of the window that has it); a tap calls [onTap].
class SecondWindow extends StatefulWidget {
  const SecondWindow({super.key, required this.onTap});

  final VoidCallback onTap;

  @override
  State<SecondWindow> createState() => _SecondWindowState();
}

class _SecondWindowState extends State<SecondWindow> {
  final TextEditingController text = TextEditingController();
  final FocusNode focus = FocusNode();

  @override
  void initState() {
    super.initState();
    text.addListener(() => setState(() {}));
  }

  @override
  void dispose() {
    text.dispose();
    focus.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) => Directionality(
    textDirection: TextDirection.ltr,
    child: GestureDetector(
      behavior: HitTestBehavior.opaque,
      onTap: widget.onTap,
      child: ColoredBox(
        color: text.text == magicWord ? typed : second,
        child: Align(
          alignment: Alignment.bottomCenter,
          child: EditableText(
            controller: text,
            focusNode: focus,
            autofocus: true,
            style: const TextStyle(fontSize: 14, color: Color(0xFFFFFFFF)),
            cursorColor: const Color(0xFFFFFFFF),
            backgroundCursorColor: const Color(0xFF000000),
          ),
        ),
      ),
    ),
  );
}
