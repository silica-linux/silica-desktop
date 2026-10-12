// La shell di Silica: un componente per processo, scelto dal primo argomento di main
// (silica-embedder --dart-arg dock). Oggi: dock.

import 'package:flutter/widgets.dart';

import 'dock/dock.dart';
import 'dock/model.dart';

/// Le app del Dock finché non arrivano i file .desktop (M3.2): come nelle riprese, 12 app,
/// separatore, cartella e cestino.
const List<Item> placeholderItems = [
  Item.app('File', running: true),
  Item.app('Navigatore', running: true),
  Item.app('Posta'),
  Item.app('Messaggi'),
  Item.app('Calendario'),
  Item.app('Note'),
  Item.app('Foto'),
  Item.app('Musica'),
  Item.app('Mappe'),
  Item.app('Editor'),
  Item.app('Terminale', running: true),
  Item.app('Impostazioni'),
  Item.separator(),
  Item.app('Download'),
  Item.app('Cestino'),
];

void main(List<String> args) {
  final component = args.isEmpty ? 'dock' : args.first;
  runApp(ShellComponent(component: component));
}

class ShellComponent extends StatelessWidget {
  const ShellComponent({super.key, required this.component});

  final String component;

  @override
  Widget build(BuildContext context) {
    final Widget child = switch (component) {
      _ => const Dock(items: placeholderItems),
    };
    return Directionality(textDirection: TextDirection.ltr, child: child);
  }
}
