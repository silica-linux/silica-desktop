// App di prova per l'embedder di Silica: colori noti, così i test leggono lo
// screenshot. Ogni tocco alterna il fondo tra [idle] e [tapped]; il quadrato in
// alto a sinistra dice il devicePixelRatio ([hiDpi] da 2 in su, altrimenti [loDpi]).
import 'package:flutter/widgets.dart';

const Color idle = Color(0xFF3366CC);
const Color tapped = Color(0xFFCC6633);
const Color loDpi = Color(0xFFFF00FF);
const Color hiDpi = Color(0xFF00FF00);
const double probeSize = 16;

void main() => runApp(const DemoApp());

class DemoApp extends StatefulWidget {
  const DemoApp({super.key});

  @override
  State<DemoApp> createState() => _DemoAppState();
}

class _DemoAppState extends State<DemoApp> {
  int taps = 0;

  @override
  Widget build(BuildContext context) {
    return Directionality(
      textDirection: TextDirection.ltr,
      child: GestureDetector(
        behavior: HitTestBehavior.opaque,
        onTap: () => setState(() => taps++),
        child: ColoredBox(
          color: taps.isEven ? idle : tapped,
          child: Stack(
            children: [
              Positioned(
                left: 0,
                top: 0,
                width: probeSize,
                height: probeSize,
                child: ColoredBox(
                  color: MediaQuery.devicePixelRatioOf(context) >= 2 ? hiDpi : loDpi,
                ),
              ),
              Center(
                child: Text(
                  'Silica · $taps',
                  style: const TextStyle(fontSize: 32, color: Color(0xFFFFFFFF)),
                ),
              ),
            ],
          ),
        ),
      ),
    );
  }
}
