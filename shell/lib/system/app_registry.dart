// The running apps that may have no window (silica-desktop#142). Closing an app's last window
// does not end the app: Ossido's window list no longer shows it, but the app registry does.
// Every app process has `$XDG_RUNTIME_DIR/silica-apps/<app id>.<pid>.sock`. An app the system
// ended while windowless leaves `<app id>.parked`, and the Dock keeps showing it as running
// until a click starts it again.

import 'dart:async';
import 'dart:io';

import 'package:flutter/foundation.dart';

/// The app ids in the registry, from the file names of its folder. A socket counts while its
/// process is alive (`alive`), and a `.parked` marker always counts.
Set<String> registeredApps(
  Iterable<String> names, {
  required bool Function(int pid) alive,
}) {
  final apps = <String>{};
  for (final name in names) {
    if (name.endsWith('.parked')) {
      final id = name.substring(0, name.length - '.parked'.length);
      if (_validAppId(id)) apps.add(id);
      continue;
    }
    if (!name.endsWith('.sock')) continue;
    final stem = name.substring(0, name.length - '.sock'.length);
    final dot = stem.lastIndexOf('.');
    if (dot <= 0) continue;
    final id = stem.substring(0, dot);
    final pid = int.tryParse(stem.substring(dot + 1));
    if (pid == null || !_validAppId(id) || !alive(pid)) continue;
    apps.add(id);
  }
  return apps;
}

bool _validAppId(String id) =>
    id.isNotEmpty &&
    id.length <= 200 &&
    !id.startsWith('.') &&
    RegExp(r'^[A-Za-z0-9._-]+$').hasMatch(id);

/// The registry, followed while the shell runs.
class AppRegistry {
  AppRegistry._();

  static final AppRegistry instance = AppRegistry._();

  /// The app ids running (with or without windows) or parked.
  final ValueNotifier<Set<String>> apps = ValueNotifier(const {});

  StreamSubscription<FileSystemEvent>? _watch;
  Timer? _poll;

  /// The registry folder, or null without a runtime folder.
  static String? get folder {
    final runtime = Platform.environment['XDG_RUNTIME_DIR'];
    return runtime == null ? null : '$runtime/silica-apps';
  }

  /// Starts following the folder: its changes, and every few seconds a process that ended
  /// without removing its socket.
  void start() {
    if (_poll != null) return;
    refresh();
    _poll = Timer.periodic(const Duration(seconds: 5), (_) => refresh());
    final path = folder;
    if (path == null) return;
    try {
      Directory(path).createSync(recursive: true);
      _watch = Directory(path).watch().listen((_) => refresh());
    } on FileSystemException {
      // No folder: polling only.
    }
  }

  void refresh() {
    final path = folder;
    if (path == null) return;
    final List<String> names;
    try {
      names = [
        for (final entity in Directory(path).listSync())
          entity.uri.pathSegments.lastWhere((s) => s.isNotEmpty),
      ];
    } on FileSystemException {
      return;
    }
    final now = registeredApps(
      names,
      alive: (pid) => Directory('/proc/$pid').existsSync(),
    );
    if (!setEquals(now, apps.value)) apps.value = now;
  }

  void stop() {
    _watch?.cancel();
    _poll?.cancel();
    _watch = null;
    _poll = null;
  }
}
