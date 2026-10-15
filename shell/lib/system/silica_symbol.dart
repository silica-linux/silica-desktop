// A silica-icons symbol (`symbols/<weight>/<name>.svg`, monochrome, stroke `currentColor`) as a
// widget, tinted. The folder comes from the same places as the app and file icons
// (`SILICA_ICONS_DIR`, then `silica/icons` in the XDG data folders).

import 'dart:io';

import 'package:flutter/widgets.dart';
import 'package:flutter_svg/flutter_svg.dart';
import 'package:silica_ui/silica_ui.dart';

final Map<String, String?> _symbolCache = {};

/// The SVG text of the symbol `name` in `weight`, or null if silica-icons has no such symbol.
String? symbolSvg(
  String name, {
  String weight = 'regular',
  Map<String, String>? env,
}) {
  final environment = env ?? Platform.environment;
  final key = '$weight/$name';
  if (env == null && _symbolCache.containsKey(key)) return _symbolCache[key];
  String? found;
  if (RegExp(r'^[a-z0-9][a-z0-9-]*$').hasMatch(name)) {
    for (final base in silicaIconDirs(environment)) {
      final file = File('$base/symbols/$weight/$name.svg');
      try {
        if (file.existsSync()) {
          found = file.readAsStringSync();
          break;
        }
      } on FileSystemException {
        continue;
      }
    }
  }
  if (env == null) _symbolCache[key] = found;
  return found;
}

/// The symbol `name` in a `size` box, drawn in `color` (nothing if the symbol is missing).
class SilicaSymbol extends StatelessWidget {
  const SilicaSymbol(
    this.name, {
    super.key,
    required this.size,
    required this.color,
    this.weight = 'regular',
    this.env,
  });

  final String name;
  final double size;
  final Color color;
  final String weight;
  final Map<String, String>? env;

  @override
  Widget build(BuildContext context) {
    final svg = symbolSvg(name, weight: weight, env: env);
    return SizedBox(
      width: size,
      height: size,
      child: svg == null
          ? null
          : SvgPicture.string(
              svg,
              width: size,
              height: size,
              colorFilter: ColorFilter.mode(color, BlendMode.srcIn),
            ),
    );
  }
}
