// The folders and files Faro finds by name (SH-SP-07, SH-SP-08): an index of the home
// folder, built in the background when Faro opens, without hidden entries and with a
// bounded depth and size.

import 'dart:io';
import 'dart:isolate';

import 'package:flutter/foundation.dart';

/// A folder or a file found by the index.
class FolderHit {
  const FolderHit({
    required this.path,
    required this.name,
    required this.parent,
    required this.modified,
    this.isFolder = true,
  });

  final bool isFolder;

  final String path;
  final String name;

  /// The name of the folder that contains it ("silica-prova-inv" in the subtitle).
  final String parent;
  final DateTime modified;
}

/// The folders under a root, refreshed in the background.
class FolderIndex {
  FolderIndex({
    required this.root,
    this.maxDepth = 5,
    this.maxFolders = 20000,
    this.maxFiles = 40000,
  });

  /// The user's home folder.
  factory FolderIndex.home() =>
      FolderIndex(root: Platform.environment['HOME'] ?? '/');

  /// An index that finds nothing (tests).
  static final FolderIndex empty = FolderIndex(root: '');

  final String root;
  final int maxDepth;
  final int maxFolders;
  final int maxFiles;
  List<FolderHit> _folders = const [];
  bool _scanning = false;

  List<FolderHit> get folders => _folders;

  /// Replaces the indexed folders (tests).
  @visibleForTesting
  void debugSetFolders(List<FolderHit> folders) => _folders = folders;

  /// Scans the root again in an isolate; `onDone` is called when the new list is in.
  Future<void> refresh({void Function()? onDone}) async {
    if (root.isEmpty || _scanning) return;
    _scanning = true;
    try {
      final root = this.root, depth = maxDepth, limit = maxFolders;
      final files = maxFiles;
      _folders = await Isolate.run(
        () => scanFolders(root, depth, limit, maxFiles: files),
      );
      onDone?.call();
    } on Object {
      // An unreadable home leaves the old list.
    } finally {
      _scanning = false;
    }
  }

  /// The entries whose name matches `query`, the best first (name starting with it, then a
  /// word starting with it, then containing it), at most `limit`. By default only folders;
  /// `files` adds the files. `under` keeps only what is inside that folder (Tab on a folder),
  /// where an empty query lists everything.
  List<FolderHit> search(
    String query, {
    int limit = 5,
    bool files = false,
    String? under,
    int minLength = 2,
  }) {
    final q = query.trim().toLowerCase();
    if (q.length < minLength) return const [];
    final prefix = under == null
        ? null
        : '${under.replaceAll(RegExp(r'/+$'), '')}/';
    final scored = <(int, FolderHit)>[];
    for (final f in _folders) {
      if (!f.isFolder && !files && under == null) continue;
      if (prefix != null && !f.path.startsWith(prefix)) continue;
      final name = f.name.toLowerCase();
      final int rank;
      if (q.isEmpty) {
        rank = f.isFolder ? 0 : 1;
      } else if (name.startsWith(q)) {
        rank = 0;
      } else if (name.split(RegExp(r'[\s\-_.]+')).any((w) => w.startsWith(q))) {
        rank = 1;
      } else if (name.contains(q)) {
        rank = 2;
      } else {
        continue;
      }
      scored.add((rank, f));
    }
    scored.sort((a, b) {
      final byRank = a.$1.compareTo(b.$1);
      return byRank != 0 ? byRank : b.$2.modified.compareTo(a.$2.modified);
    });
    return [for (final (_, f) in scored.take(limit)) f];
  }
}

/// The non-hidden folders under `root`, breadth first, up to `maxDepth` levels and
/// `maxFolders` folders, and their non-hidden files up to `maxFiles`. Unreadable folders are
/// skipped.
List<FolderHit> scanFolders(
  String root,
  int maxDepth,
  int maxFolders, {
  int maxFiles = 0,
}) {
  final found = <FolderHit>[];
  var files = 0;
  var level = <Directory>[Directory(root)];
  for (var depth = 0; depth < maxDepth && level.isNotEmpty; depth++) {
    final next = <Directory>[];
    for (final dir in level) {
      final List<FileSystemEntity> children;
      try {
        children = dir.listSync(followLinks: false);
      } on FileSystemException {
        continue;
      }
      for (final child in children) {
        final isFolder = child is Directory;
        if (!isFolder && (child is! File || files >= maxFiles)) continue;
        final name = child.path
            .split('/')
            .lastWhere((s) => s.isNotEmpty, orElse: () => '');
        if (name.isEmpty || name.startsWith('.')) continue;
        DateTime modified;
        try {
          modified = child.statSync().modified;
        } on FileSystemException {
          continue;
        }
        final parentPath = dir.path;
        final parent = parentPath
            .split('/')
            .lastWhere((s) => s.isNotEmpty, orElse: () => '/');
        found.add(
          FolderHit(
            path: child.path,
            name: name,
            parent: parent,
            modified: modified,
            isFolder: isFolder,
          ),
        );
        if (!isFolder) {
          files++;
          continue;
        }
        next.add(child);
        if (found.length - files >= maxFolders) return found;
      }
    }
    level = next;
  }
  return found;
}
