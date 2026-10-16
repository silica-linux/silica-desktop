// The settings of a folder in the Dock (dock.md › D-04, dock-stacks.md): order, how the icon
// shows (folder or stack) and how the content opens (fan, grid, list, automatic). Defaults
// for an added folder: by name, stack, automatic. Saved by path in
// ~/.config/silica/dock-stacks.json.

import 'dart:convert';
import 'dart:io';

import 'apps.dart' show downloadDir;

/// The order of the files in the stack.
enum StackSort { name, added, modified, created, kind }

/// The icon in the Dock: the folder or the stacked documents.
enum StackDisplay { folder, stack }

/// How the content opens.
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

/// The settings of a folder nobody has configured: by name, except the Downloads folder, by
/// Date Added (newest first, like a download stack).
StackSettings defaultStackSettings(String folder, {Map<String, String>? env}) {
  final e = env ?? Platform.environment;
  final home = e['HOME'];
  if (home != null && folder == downloadDir(home, env: e)) {
    return const StackSettings(sort: StackSort.added);
  }
  return const StackSettings();
}

File _file(Map<String, String>? env) {
  final e = env ?? Platform.environment;
  final config = e['XDG_CONFIG_HOME'] ?? '${e['HOME'] ?? ''}/.config';
  return File('$config/silica/dock-stacks.json');
}

/// The settings of all folders, by path.
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
    // Broken configuration: the defaults.
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
    // Without a writable configuration the choice lasts until the end of the session.
  }
}
