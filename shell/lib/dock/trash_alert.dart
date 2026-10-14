// The "Empty Trash" alert (dock.md, D-07): a 292 × 268 window centered horizontally,
// with its top at ~20 % of the screen (295 over 1440); thick glass (`#c5c6ca` over grey),
// radius ~26; a 66 pt Trash icon at (19, 19), main text in bold at (21, 99) 250 wide,
// secondary text at (21, 157), two full-width 262 × 30 buttons at (15, 189) and (15, 223):
// the blue action above, Cancel translucent white below.

import 'dart:io';

import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';
import 'package:silica_ui/silica_ui.dart';

import '../faro/faro.dart' show OuterShadow;
import '../l10n/l10n.dart';

const Size trashAlertSize = Size(292, 268);

/// The top of the alert, as a fraction of the screen height.
const double trashAlertTop = 295 / 1440;
const double trashAlertRadius = 26;
const double trashAlertIcon = 66;
const double trashAlertButtonWidth = 262;
const double trashAlertButtonHeight = 30;
const double trashAlertButtonsTop = 189;
const double trashAlertButtonGap = 4;

const Color alertText = Color(0xFF1A1A1A);
const Color alertDefault = Color(0xFF007AFF);

/// `#e5e4e5` over the `#c8c9ce` glass: white at ~55 %.
const Color alertCancel = Color(0x8CFFFFFF);
const Color alertCancelText = Color(0xFF242424);

/// Empties the user's Trash: `gio trash --empty` (all trashes, including volumes); without
/// `gio`, the `files` and `info` folders of the home Trash (freedesktop specification).
Future<void> emptyTrash({Map<String, String>? env}) async {
  // gio always works on the real user's Trash: never with a given environment or under
  // `flutter test`, where it would empty the Trash of the machine running the tests.
  if (env == null && !Platform.environment.containsKey('FLUTTER_TEST')) {
    try {
      final result = await Process.run('gio', ['trash', '--empty']);
      if (result.exitCode == 0) return;
    } on ProcessException {
      // No gio: by hand.
    }
  }
  if (env == null && Platform.environment.containsKey('FLUTTER_TEST')) return;
  final home = (env ?? Platform.environment)['HOME'] ?? '';
  for (final name in ['files', 'info']) {
    final dir = Directory('$home/.local/share/Trash/$name');
    if (!dir.existsSync()) continue;
    for (final entry in dir.listSync()) {
      try {
        entry.deleteSync(recursive: true);
      } on FileSystemException {
        // A file that cannot be removed stays: the rest is emptied anyway.
      }
    }
  }
}

/// Moves `paths` to the Trash: `gio trash` (also on volumes); without `gio`, into the home
/// Trash with the `.trashinfo` file (freedesktop Trash specification).
Future<void> moveToTrash(
  List<String> paths, {
  Map<String, String>? env,
  DateTime? now,
}) async {
  if (paths.isEmpty) return;
  if (env == null && Platform.environment.containsKey('FLUTTER_TEST')) return;
  if (env == null) {
    try {
      final result = await Process.run('gio', ['trash', '--', ...paths]);
      if (result.exitCode == 0) return;
    } on ProcessException {
      // No gio: by hand.
    }
  }
  final home = (env ?? Platform.environment)['HOME'] ?? '';
  final files = Directory('$home/.local/share/Trash/files')
    ..createSync(recursive: true);
  final info = Directory('$home/.local/share/Trash/info')
    ..createSync(recursive: true);
  final when = (now ?? DateTime.now()).toIso8601String().split('.').first;
  for (final path in paths) {
    final base = path.split('/').where((p) => p.isNotEmpty).lastOrNull;
    if (base == null) continue;
    var name = base;
    for (var n = 2; File('${info.path}/$name.trashinfo').existsSync(); n++) {
      name = '$base $n';
    }
    try {
      File('${info.path}/$name.trashinfo').writeAsStringSync(
        '[Trash Info]\nPath=${Uri.file(path).path}\nDeletionDate=$when\n',
      );
      FileSystemEntity.typeSync(path, followLinks: false) ==
              FileSystemEntityType.directory
          ? Directory(path).renameSync('${files.path}/$name')
          : File(path).renameSync('${files.path}/$name');
    } on FileSystemException {
      // Another volume or a protected file: it stays where it is.
      try {
        File('${info.path}/$name.trashinfo').deleteSync();
      } on FileSystemException {
        // Not even the info file was written.
      }
    }
  }
}

class TrashAlert extends StatelessWidget {
  const TrashAlert({super.key, required this.onEmpty, required this.onCancel});

  final VoidCallback onEmpty;
  final VoidCallback onCancel;

  @override
  Widget build(BuildContext context) {
    // In dark like the silica_ui alerts (misc.md): text `#ebebeb`, Cancel `#525253`
    // with text `#e0e0e0`.
    final dark =
        MediaQuery.maybePlatformBrightnessOf(context) == Brightness.dark;
    final text = dark ? AlertColors.textDark : alertText;
    final icon = findSilicaIcon('trash-full');
    return Focus(
      autofocus: true,
      onKeyEvent: (node, event) {
        if (event is! KeyDownEvent) return KeyEventResult.ignored;
        if (event.logicalKey == LogicalKeyboardKey.escape) {
          onCancel();
          return KeyEventResult.handled;
        }
        if (event.logicalKey == LogicalKeyboardKey.enter ||
            event.logicalKey == LogicalKeyboardKey.numpadEnter) {
          onEmpty();
          return KeyEventResult.handled;
        }
        return KeyEventResult.ignored;
      },
      child: OuterShadow(
        radius: trashAlertRadius,
        blurRadius: 40,
        offset: const Offset(0, 10),
        color: const Color(0x4D000000),
        child: LiquidGlass(
          radius: trashAlertRadius,
          material: GlassMaterial.thick,
          interactive: false,
          child: Stack(
            children: [
              Positioned(
                left: 19,
                top: 19,
                width: trashAlertIcon,
                height: trashAlertIcon,
                child: icon == null
                    ? const SizedBox()
                    : SilicaIcon(
                        LayeredSource(icon),
                        side: trashAlertIcon,
                        dark: false,
                        besideApps: true,
                      ),
              ),
              Positioned(
                left: 21,
                width: 250,
                top: 99,
                child: Text(
                  l10nOf(context).trashAlertTitle,
                  maxLines: 3,
                  style: TextStyle(
                    fontFamily: uiFontFamily,
                    package: 'silica_ui',
                    fontSize: 13,
                    fontWeight: FontWeight.w600,
                    color: text,
                    height: 16.7 / 13,
                  ),
                ),
              ),
              Positioned(
                left: 21,
                width: 250,
                top: 157,
                child: Text(
                  l10nOf(context).trashAlertMessage,
                  style: uiText(size: 13, color: text),
                ),
              ),
              _button(
                l10nOf(context).emptyTrash,
                trashAlertButtonsTop,
                alertDefault,
                const Color(0xFFFFFFFF),
                onEmpty,
              ),
              _button(
                l10nOf(context).cancel,
                trashAlertButtonsTop +
                    trashAlertButtonHeight +
                    trashAlertButtonGap,
                dark ? AlertColors.normalDark : alertCancel,
                dark ? AlertColors.normalTextDark : alertCancelText,
                onCancel,
              ),
            ],
          ),
        ),
      ),
    );
  }

  Widget _button(
    String label,
    double top,
    Color fill,
    Color text,
    VoidCallback onTap,
  ) => Positioned(
    left: (trashAlertSize.width - trashAlertButtonWidth) / 2,
    top: top,
    width: trashAlertButtonWidth,
    height: trashAlertButtonHeight,
    child: GestureDetector(
      onTap: onTap,
      child: DecoratedBox(
        decoration: BoxDecoration(
          color: fill,
          borderRadius: BorderRadius.circular(trashAlertButtonHeight / 2),
        ),
        child: Center(
          child: Text(label, style: uiText(size: 13, color: text)),
        ),
      ),
    ),
  );
}
