// L'avviso "Svuota il Cestino" (dock.md, D-07): finestra 292 × 268 centrata in orizzontale,
// con la cima al ~20 % dello schermo (295 su 1440); vetro spesso (`#c5c6ca` sul grigio),
// raggio ~26; icona del Cestino da 66 pt a (19, 19), testo principale in grassetto a
// (21, 99) largo 250, testo secondario a (21, 157), due pulsanti a tutta larghezza da
// 262 × 30 a (15, 189) e (15, 223): l'azione blu sopra, Annulla bianco traslucido sotto.

import 'dart:io';

import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';
import 'package:silica_ui/silica_ui.dart';

import '../faro/faro.dart' show OuterShadow;

const Size trashAlertSize = Size(292, 268);

/// La cima dell'avviso, in frazione dell'altezza dello schermo.
const double trashAlertTop = 295 / 1440;
const double trashAlertRadius = 26;
const double trashAlertIcon = 66;
const double trashAlertButtonWidth = 262;
const double trashAlertButtonHeight = 30;
const double trashAlertButtonsTop = 189;
const double trashAlertButtonGap = 4;

const Color alertText = Color(0xFF1A1A1A);
const Color alertDefault = Color(0xFF007AFF);

/// `#e5e4e5` sul vetro `#c8c9ce`: bianco al ~55 %.
const Color alertCancel = Color(0x8CFFFFFF);
const Color alertCancelText = Color(0xFF242424);

/// Svuota il Cestino dell'utente: `gio trash --empty` (tutti i cestini, anche dei volumi);
/// senza `gio`, le cartelle `files` e `info` del Cestino della home (specifica freedesktop).
Future<void> emptyTrash({Map<String, String>? env}) async {
  try {
    final result = await Process.run('gio', ['trash', '--empty']);
    if (result.exitCode == 0) return;
  } on ProcessException {
    // Nessun gio: a mano.
  }
  final home = (env ?? Platform.environment)['HOME'] ?? '';
  for (final name in ['files', 'info']) {
    final dir = Directory('$home/.local/share/Trash/$name');
    if (!dir.existsSync()) continue;
    for (final entry in dir.listSync()) {
      try {
        entry.deleteSync(recursive: true);
      } on FileSystemException {
        // Un file che non si può togliere resta: il resto si svuota comunque.
      }
    }
  }
}

/// Sposta `paths` nel Cestino: `gio trash` (anche sui volumi); senza `gio`, nel Cestino della
/// home con il file `.trashinfo` (specifica freedesktop Trash).
Future<void> moveToTrash(
  List<String> paths, {
  Map<String, String>? env,
  DateTime? now,
}) async {
  if (paths.isEmpty) return;
  if (env == null) {
    try {
      final result = await Process.run('gio', ['trash', '--', ...paths]);
      if (result.exitCode == 0) return;
    } on ProcessException {
      // Nessun gio: a mano.
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
      // Un altro volume o un file protetto: resta dov'è.
      try {
        File('${info.path}/$name.trashinfo').deleteSync();
      } on FileSystemException {
        // Nemmeno la scheda si è scritta.
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
    // In scuro come gli avvisi di silica_ui (misc.md): testo `#ebebeb`, Annulla `#525253`
    // con il testo `#e0e0e0`.
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
                  'Vuoi eliminare definitivamente gli elementi nel Cestino?',
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
                  'Non si potrà annullare.',
                  style: uiText(size: 13, color: text),
                ),
              ),
              _button(
                'Svuota il Cestino',
                trashAlertButtonsTop,
                alertDefault,
                const Color(0xFFFFFFFF),
                onEmpty,
              ),
              _button(
                'Annulla',
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
