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

  @override
  void initState() {
    super.initState();
    text.addListener(() => setState(() {}));
    HardwareKeyboard.instance.addHandler(_onKey);
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
    return false;
  }

  Color get background {
    if (text.text == magicWord) return typed;
    return taps.isEven ? idle : tapped;
  }

  @override
  Widget build(BuildContext context) {
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
