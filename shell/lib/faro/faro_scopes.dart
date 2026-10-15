// Faro's scopes: the four round buttons (apps, files, actions, clipboard) and the kinds of
// rows they list (SH-SP-05, SH-SP-15, SH-SP-16).
// - Apps: the Launcher.
// - Files: files and folders of the home folder, or of the folder chosen with Tab.
// - Actions: the system actions of the command catalog (silica-daemon,
//   dev.frascella.Silica.Actions1) and the apps' own actions (.desktop Actions).
// - Clipboard: the clipboard history (silica-daemon, dev.frascella.Silica.Clipboard1).
// The files scope is a 640 x 547 pt grid of 5 columns; the actions scope lists everything
// in sections per app, sorted alphabetically. The clipboard scope's rows reuse the measured
// result rows (PROVISIONAL: it was not measured).

part of 'faro.dart';

/// What Faro searches.
enum FaroScope { all, files, actions, clipboard }

/// The scope Cmd+2..4 open (Cmd+1 is the Launcher).
FaroScope? scopeOfDigit(LogicalKeyboardKey key) {
  if (key == LogicalKeyboardKey.digit2) return FaroScope.files;
  if (key == LogicalKeyboardKey.digit3) return FaroScope.actions;
  if (key == LogicalKeyboardKey.digit4) return FaroScope.clipboard;
  return null;
}

/// A file found by the index (folders are [FolderItem]).
class FileItem extends FaroItem {
  const FileItem(this.file);
  final FolderHit file;
}

/// A system action of the command catalog.
class ActionItem extends FaroItem {
  const ActionItem(this.action);
  final SystemAction action;
}

/// An action of an app (`[Desktop Action …]`).
class AppActionItem extends FaroItem {
  const AppActionItem(this.entry, this.action);
  final DesktopEntry entry;
  final DesktopAction action;
}

/// An entry of the clipboard history.
class ClipItem extends FaroItem {
  const ClipItem(this.clip);
  final ClipEntry clip;
}

/// "Clear History" at the end of the clipboard list.
class ClearClipboardItem extends FaroItem {
  const ClearClipboardItem();
}

@immutable
class SystemAction {
  const SystemAction({required this.id, required this.label, this.shortcut});

  /// From the daemon's JSON (`{id, label, description, shortcut}`).
  static SystemAction? fromJson(Object? json) {
    if (json is! Map) return null;
    final id = json['id'];
    final label = json['label'];
    if (id is! String || label is! String) return null;
    final shortcut = json['shortcut'];
    return SystemAction(
      id: id,
      label: label,
      shortcut: shortcut is String && shortcut.isNotEmpty ? shortcut : null,
    );
  }

  final String id;
  final String label;

  /// As the catalog writes it (`Logo+Shift+3`).
  final String? shortcut;
}

enum ClipKind { text, image, files }

@immutable
class ClipEntry {
  const ClipEntry({
    required this.id,
    required this.kind,
    required this.text,
    required this.time,
    this.image,
  });

  /// From the daemon's JSON (`{id, kind, text, image?, time}`).
  static ClipEntry? fromJson(Object? json) {
    if (json is! Map) return null;
    final id = json['id'];
    final kind = switch (json['kind']) {
      'text' => ClipKind.text,
      'image' => ClipKind.image,
      'files' => ClipKind.files,
      _ => null,
    };
    final time = json['time'];
    if (id is! int || kind == null) return null;
    final text = json['text'];
    final image = json['image'];
    return ClipEntry(
      id: id,
      kind: kind,
      text: text is String ? text : '',
      time: DateTime.fromMillisecondsSinceEpoch(
        (time is int ? time : 0) * 1000,
      ),
      image: image is String ? image : null,
    );
  }

  final int id;
  final ClipKind kind;
  final String text;
  final DateTime time;
  final String? image;

  /// The files of a file list.
  List<String> get paths => [
    for (final line in text.split('\n'))
      if (Uri.tryParse(line.trim()) case final uri? when uri.scheme == 'file')
        uri.toFilePath(),
  ];

  /// One line to show: the text on a single line, or the file names.
  String get title => switch (kind) {
    ClipKind.text => text.trim().replaceAll(RegExp(r'\s+'), ' '),
    ClipKind.files => paths.map((p) => p.split('/').last).join(', '),
    ClipKind.image => '',
  };
}

const String _daemon = 'dev.frascella.Silica';

/// The system actions, read once from silica-daemon.
class SystemActions {
  List<SystemAction> _actions = const [];
  bool _loaded = false;

  List<SystemAction> get actions => _actions;

  Future<void> load({void Function()? onDone}) async {
    if (_loaded) return;
    try {
      final reply = await DBus.instance.call(
        destination: _daemon,
        path: '/dev/frascella/Silica/Actions1',
        interface: 'dev.frascella.Silica.Actions1',
        method: 'List',
      );
      final json = reply.isEmpty ? null : jsonDecode('${reply.first}');
      _actions = [
        if (json is List)
          for (final a in json) ?SystemAction.fromJson(a),
      ];
      _loaded = true;
      onDone?.call();
    } on Object {
      // Without the daemon the actions scope has the apps' actions only.
    }
  }

  /// Replaces the list (tests).
  @visibleForTesting
  void debugSet(List<SystemAction> actions) {
    _actions = actions;
    _loaded = true;
  }
}

/// The clipboard history, kept in sync with silica-daemon's `Changed` signal.
class ClipboardHistory {
  List<ClipEntry> _entries = const [];
  StreamSubscription<DBusSignal>? _changes;
  void Function()? _onChange;

  List<ClipEntry> get entries => _entries;

  static const String _path = '/dev/frascella/Silica/Clipboard1';
  static const String _interface = 'dev.frascella.Silica.Clipboard1';

  Future<void> load({void Function()? onChange}) async {
    _onChange = onChange;
    _changes ??= DBus.instance
        .signals(path: _path, interface: _interface, member: 'Changed')
        .listen((_) => _refresh());
    await _refresh();
  }

  Future<void> _refresh() async {
    try {
      final reply = await DBus.instance.call(
        destination: _daemon,
        path: _path,
        interface: _interface,
        method: 'History',
      );
      final json = reply.isEmpty ? null : jsonDecode('${reply.first}');
      _entries = [
        if (json is List)
          for (final e in json) ?ClipEntry.fromJson(e),
      ];
      _onChange?.call();
    } on Object {
      // No history without the daemon.
    }
  }

  /// Puts an entry back on the clipboard.
  Future<void> copy(ClipEntry entry) => _call('Copy', 't', [entry.id]);

  Future<void> remove(ClipEntry entry) => _call('Remove', 't', [entry.id]);

  Future<void> clear() => _call('Clear', '', const []);

  Future<void> _call(
    String method,
    String signature,
    List<Object?> args,
  ) async {
    try {
      await DBus.instance.call(
        destination: _daemon,
        path: _path,
        interface: _interface,
        method: method,
        signature: signature,
        args: args,
      );
    } on Object {
      // The daemon is gone: nothing to do.
    }
  }

  void dispose() => _changes?.cancel();

  /// Replaces the entries (tests).
  @visibleForTesting
  void debugSet(List<ClipEntry> entries) => _entries = entries;
}

/// The rows of a scope for `query`.
List<FaroItem> scopeItems(
  FaroScope scope, {
  required String query,
  required Map<String, DesktopEntry> entries,
  required FolderIndex folders,
  required SystemActions actions,
  required ClipboardHistory clipboard,
  String Function(SystemAction)? labelOf,
  String? under,
  String systemSection = 'System',
}) {
  final q = query.trim().toLowerCase();
  bool matches(String text) => q.isEmpty || text.toLowerCase().contains(q);
  switch (scope) {
    case FaroScope.all:
      return const [];
    case FaroScope.files:
      // Nothing typed: the recent files and folders fill the grid.
      return [
        for (final hit in folders.search(
          query,
          limit: 40,
          files: true,
          under: under,
          minLength: 0,
        ))
          hit.isFolder ? FolderItem(hit) : FileItem(hit),
      ];
    case FaroScope.actions:
      // Sections per app, alphabetically; inside a section, alphabetically by action.
      final rows = <(String, String, FaroItem)>[
        for (final a in actions.actions)
          if (matches(labelOf?.call(a) ?? a.label))
            (systemSection, labelOf?.call(a) ?? a.label, ActionItem(a)),
        for (final e in entries.values)
          for (final a in e.actions)
            if (matches('${e.name} ${a.name}'))
              (e.name, a.name, AppActionItem(e, a)),
      ];
      rows.sort((x, y) {
        final bySection = x.$1.toLowerCase().compareTo(y.$1.toLowerCase());
        return bySection != 0
            ? bySection
            : x.$2.toLowerCase().compareTo(y.$2.toLowerCase());
      });
      return [for (final r in rows) r.$3];
    case FaroScope.clipboard:
      final clips = [
        for (final c in clipboard.entries)
          if (matches(c.kind == ClipKind.image ? 'image' : c.text)) ClipItem(c),
      ];
      return [...clips, if (clips.isNotEmpty) const ClearClipboardItem()];
  }
}

/// The catalog writes shortcuts as `Logo+Shift+3`: the key glyphs of the reference.
String shortcutGlyphs(String shortcut) => shortcut
    .split('+')
    .map(
      (part) => switch (part) {
        'Logo' || 'Super' => '⌘',
        'Shift' => '⇧',
        'Ctrl' || 'Control' => '⌃',
        'Alt' => '⌥',
        'Escape' => '⎋',
        'Return' => '↩',
        'Space' => 'Space',
        _ => part.length == 1 ? part.toUpperCase() : part,
      },
    )
    .join();
