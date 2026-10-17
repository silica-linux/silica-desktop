// The shell's scroll behavior: no platform scroll bar. Flutter's default adds a desktop bar
// to every scroll view; the shell draws silica_ui's AmalfiScrollbar explicitly, where a
// surface scrolls, so every surface has the same one (silica-ui#93).

import 'package:flutter/widgets.dart';

class ShellScrollBehavior extends ScrollBehavior {
  const ShellScrollBehavior();

  @override
  Widget buildScrollbar(
    BuildContext context,
    Widget child,
    ScrollableDetails details,
  ) => child;
}
