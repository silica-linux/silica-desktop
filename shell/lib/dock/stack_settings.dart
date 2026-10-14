// Le impostazioni di una cartella nel Dock (dock.md › D-04, dock-stacks.md): ordine, come si
// mostra l'icona (cartella o pila) e come si apre il contenuto (ventaglio, griglia, elenco,
// automatico). Predefinite per una cartella aggiunta: per nome, pila, automatico. Salvate per
// percorso in ~/.config/silica/dock-stacks.json.

import 'dart:convert';
import 'dart:io';

/// L'ordine dei file nella pila.
enum StackSort { name, added, modified, created, kind }

/// L'icona nel Dock: la cartella o i documenti sovrapposti.
enum StackDisplay { folder, stack }

/// Come si apre il contenuto.
enum StackContent { fan, grid, list, automatic }

class StackSettings {
  const StackSettings({
    this.sort = StackSort.name,
    this.display = StackDisplay.stack,
    this.content = StackContent.automatic,
  });

  final StackSort sort;
  final StackDisplay display;
  final StackContent content;

  StackSettings copyWith({
    StackSort? sort,
    StackDisplay? display,
    StackContent? content,
  }) => StackSettings(
    sort: sort ?? this.sort,
    display: display ?? this.display,
    content: content ?? this.content,
  );

  Map<String, String> toJson() => {
    'sort': sort.name,
    'display': display.name,
    'content': content.name,
  };

  static StackSettings fromJson(Object? json) {
    if (json is! Map) return const StackSettings();
    T pick<T extends Enum>(List<T> values, Object? name, T fallback) =>
        values.where((v) => v.name == name).firstOrNull ?? fallback;
    return StackSettings(
      sort: pick(StackSort.values, json['sort'], StackSort.name),
      display: pick(StackDisplay.values, json['display'], StackDisplay.stack),
      content: pick(
        StackContent.values,
        json['content'],
        StackContent.automatic,
      ),
    );
  }
}

File _file(Map<String, String>? env) {
  final e = env ?? Platform.environment;
  final config = e['XDG_CONFIG_HOME'] ?? '${e['HOME'] ?? ''}/.config';
  return File('$config/silica/dock-stacks.json');
}

/// Le impostazioni di tutte le cartelle, per percorso.
Map<String, StackSettings> loadStackSettings({Map<String, String>? env}) {
  final file = _file(env);
  try {
    if (!file.existsSync()) return {};
    final json = jsonDecode(file.readAsStringSync());
    if (json is! Map) return {};
    return {
      for (final MapEntry(:key, :value) in json.entries)
        '$key': StackSettings.fromJson(value),
    };
  } on FileSystemException {
    return {};
  } on FormatException {
    // Configurazione rovinata: le predefinite.
    return {};
  }
}

void saveStackSettings(
  Map<String, StackSettings> settings, {
  Map<String, String>? env,
}) {
  final file = _file(env);
  try {
    file.parent.createSync(recursive: true);
    file.writeAsStringSync(
      jsonEncode({
        for (final MapEntry(:key, :value) in settings.entries)
          key: value.toJson(),
      }),
    );
  } on FileSystemException {
    // Senza configurazione scrivibile la scelta vale fino alla fine della sessione.
  }
}
