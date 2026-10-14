// The screenshot settings: where captures go, the timer, the thumbnail, the pointer, the
// mode and selection the ⇧⌘5 bar remembers and where the bar was left. Kept in
// $XDG_CONFIG_HOME/silica/screenshot.json; ⇧⌘3 and ⇧⌘4 follow the same place and thumbnail
// choices as the bar (as on the reference system).

import 'dart:convert';
import 'dart:io';
import 'dart:ui' show Offset, Rect;

/// Where a capture goes.
sealed class ShotDestination {
  const ShotDestination();

  /// From its saved form: `desktop`, `documents`, `clipboard`, `folder:<path>`, `app:<id>`.
  static ShotDestination parse(Object? text) {
    if (text is! String) return const ToDesktop();
    if (text == 'documents') return const ToDocuments();
    if (text == 'clipboard') return const ToClipboard();
    if (text.startsWith('folder:') && text.length > 'folder:'.length) {
      return ToFolder(text.substring('folder:'.length));
    }
    if (text.startsWith('app:') && text.length > 'app:'.length) {
      return ToApp(text.substring('app:'.length));
    }
    return const ToDesktop();
  }

  String get saved;

  @override
  bool operator ==(Object other) =>
      other is ShotDestination && other.saved == saved;

  @override
  int get hashCode => saved.hashCode;
}

class ToDesktop extends ShotDestination {
  const ToDesktop();
  @override
  String get saved => 'desktop';
}

class ToDocuments extends ShotDestination {
  const ToDocuments();
  @override
  String get saved => 'documents';
}

class ToClipboard extends ShotDestination {
  const ToClipboard();
  @override
  String get saved => 'clipboard';
}

/// A folder chosen with "Other Folder…".
class ToFolder extends ShotDestination {
  const ToFolder(this.path);
  final String path;
  @override
  String get saved => 'folder:$path';
}

/// An app that opens the image (the default image viewer), by its desktop entry id.
class ToApp extends ShotDestination {
  const ToApp(this.id);
  final String id;
  @override
  String get saved => 'app:$id';
}

/// The bar's capture modes, left to right.
enum ToolbarChoice { screen, window, selection }

/// The timer before a capture from the bar.
const List<int> timerChoices = [0, 5, 10];

class ScreenshotSettings {
  ScreenshotSettings({
    this.destination = const ToDesktop(),
    this.timer = 0,
    this.showThumbnail = true,
    this.rememberSelection = true,
    this.showPointer = false,
    this.mode = ToolbarChoice.selection,
    this.selection,
    this.barCenter,
    List<String>? recentFolders,
  }) : recentFolders = recentFolders ?? [];

  ShotDestination destination;

  /// Seconds before the capture (0, 5 or 10).
  int timer;
  bool showThumbnail;

  /// The bar opens with the last selection (otherwise a default one).
  bool rememberSelection;

  /// The pointer drawn into the captures.
  bool showPointer;

  /// The bar's mode, kept from one opening to the next.
  ToolbarChoice mode;

  /// The last selection of the bar, in points.
  Rect? selection;

  /// Where the bar was left (its center), in points; null: centered above the Dock.
  Offset? barCenter;

  /// Folders chosen with "Other Folder…", newest first (at most 3, provisional).
  final List<String> recentFolders;

  static const int maxRecentFolders = 3;

  Map<String, Object?> toJson() => {
    'destination': destination.saved,
    'timer': timer,
    'thumbnail': showThumbnail,
    'remember_selection': rememberSelection,
    'pointer': showPointer,
    'mode': mode.name,
    if (selection case final s?)
      'selection': [s.left, s.top, s.width, s.height],
    if (barCenter case final c?) 'bar': [c.dx, c.dy],
    'recent_folders': recentFolders,
  };

  static ScreenshotSettings fromJson(Object? json) {
    if (json is! Map) return ScreenshotSettings();
    bool flag(String key, bool fallback) =>
        json[key] is bool ? json[key] as bool : fallback;
    List<double>? numbers(Object? value, int count) {
      if (value is! List || value.length != count) return null;
      final out = <double>[];
      for (final v in value) {
        if (v is! num || !v.isFinite) return null;
        out.add(v.toDouble());
      }
      return out;
    }

    final rect = numbers(json['selection'], 4);
    final bar = numbers(json['bar'], 2);
    final timer = json['timer'];
    return ScreenshotSettings(
      destination: ShotDestination.parse(json['destination']),
      timer: timer is int && timerChoices.contains(timer) ? timer : 0,
      showThumbnail: flag('thumbnail', true),
      rememberSelection: flag('remember_selection', true),
      showPointer: flag('pointer', false),
      mode:
          ToolbarChoice.values
              .where((m) => m.name == json['mode'])
              .firstOrNull ??
          ToolbarChoice.selection,
      selection: rect != null && rect[2] > 0 && rect[3] > 0
          ? Rect.fromLTWH(rect[0], rect[1], rect[2], rect[3])
          : null,
      barCenter: bar == null ? null : Offset(bar[0], bar[1]),
      recentFolders: [
        if (json['recent_folders'] case final List list)
          for (final f in list.take(maxRecentFolders))
            if (f is String && f.startsWith('/')) f,
      ],
    );
  }

  /// A folder chosen with "Other Folder…": it becomes the destination and the first of the
  /// recent ones.
  void chooseFolder(String path) {
    destination = ToFolder(path);
    recentFolders
      ..remove(path)
      ..insert(0, path);
    if (recentFolders.length > maxRecentFolders) {
      recentFolders.removeRange(maxRecentFolders, recentFolders.length);
    }
  }

  /// The settings file: $XDG_CONFIG_HOME/silica/screenshot.json.
  static File file({Map<String, String>? env}) {
    final e = env ?? Platform.environment;
    final home = e['HOME'] ?? '/tmp';
    final config = e['XDG_CONFIG_HOME'] ?? '$home/.config';
    return File('$config/silica/screenshot.json');
  }

  static ScreenshotSettings load(File file) {
    try {
      return fromJson(jsonDecode(file.readAsStringSync()));
    } on FileSystemException {
      return ScreenshotSettings();
    } on FormatException {
      return ScreenshotSettings();
    }
  }

  void save(File file) {
    try {
      file.parent.createSync(recursive: true);
      file.writeAsStringSync(jsonEncode(toJson()));
    } on FileSystemException {
      // Settings that cannot be written stay for this session.
    }
  }
}
