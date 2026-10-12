/// Design system di Silica per Flutter.
library;

import 'package:flutter/widgets.dart';

export 'src/glass.dart';

/// Il font dell'interfaccia: Inter (OFL), al posto dei font di sistema di Apple.
const String uiFontFamily = 'Inter';

/// Uno stile di testo in Inter, servito da questo pacchetto.
TextStyle uiText({required double size, required Color color}) =>
    TextStyle(fontFamily: uiFontFamily, package: 'silica_ui', fontSize: size, color: color);
