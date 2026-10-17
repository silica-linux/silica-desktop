// Folder colour and symbol, as the file manager (Mustache) stores them on the folder itself:
//
// - colour tags in `user.xdg.tags` (comma-separated, the freedesktop convention): a tag named
//   like a colour (Red, Rosso…) tints the folder;
// - a custom tint and/or symbol in `user.silica.folder`, JSON such as
//   `{"tint":"#e8336d","symbol":"music-note"}` (tint `#rrggbb` or a colour name, symbol a
//   silica-icons symbol name);
// - on disks without user attributes, the same JSON in the GIO metadata store
//   (`metadata::silica-folder`).
//
// The tint is the custom one, else the first colour tag's, else none. The Dock reads them for
// its folder items and the folders inside stacks; values are cached per folder and dropped when
// the Dock's folder watches report a change (an attribute change arrives as a modify event).

import 'dart:convert';
import 'dart:ffi';
import 'dart:io';

import 'package:ffi/ffi.dart';
import 'package:flutter/foundation.dart';
import 'package:silica_ui/silica_ui.dart';

const String folderStyleAttribute = 'user.silica.folder';
const String folderTagsAttribute = 'user.xdg.tags';
const String folderStyleMetadata = 'metadata::silica-folder';

/// The colour names of silica-icons (`labels` of the generic folder) and the tag names that
/// mean them, in every language the file manager writes.
const Map<String, List<String>> colourTagNames = {
  'red': ['Red', 'Rosso'],
  'orange': ['Orange', 'Arancio'],
  'yellow': ['Yellow', 'Giallo'],
  'green': ['Green', 'Verde'],
  'blue': ['Blue', 'Blu'],
  'purple': ['Purple', 'Viola'],
  'gray': ['Gray', 'Grigio'],
};

/// The colour name of a tag, or null for a tag without a colour.
String? tagColourName(String tag) {
  for (final MapEntry(:key, :value) in colourTagNames.entries) {
    if (value.contains(tag)) return key;
  }
  return null;
}

final RegExp _hexTint = RegExp(r'^#[0-9a-fA-F]{6}$');
final RegExp _symbolName = RegExp(r'^[a-z0-9][a-z0-9-]{0,63}$');

/// What the user chose for a folder.
@immutable
class FolderStyle {
  const FolderStyle({this.tint, this.symbol});

  final String? tint;
  final String? symbol;

  /// The stored JSON; malformed fields are dropped. Null: nothing usable.
  static FolderStyle? parse(String? json) {
    if (json == null || json.trim().isEmpty) return null;
    try {
      final value = jsonDecode(json);
      if (value is! Map) return null;
      final tint = value['tint'];
      final symbol = value['symbol'];
      final validTint =
          tint is String &&
          (_hexTint.hasMatch(tint) || colourTagNames.containsKey(tint));
      final validSymbol = symbol is String && _symbolName.hasMatch(symbol);
      if (!validTint && !validSymbol) return null;
      return FolderStyle(
        tint: validTint ? tint.toLowerCase() : null,
        symbol: validSymbol ? symbol : null,
      );
    } on FormatException {
      return null;
    }
  }

  @override
  bool operator ==(Object other) =>
      other is FolderStyle && other.tint == tint && other.symbol == symbol;

  @override
  int get hashCode => Object.hash(tint, symbol);
}

/// The value of `metadata::silica-folder` in the output of `gio info -a`.
String? parseGioInfo(String output) {
  const prefix = '$folderStyleMetadata: ';
  for (final line in const LineSplitter().convert(output)) {
    final t = line.trim();
    if (t.startsWith(prefix)) return t.substring(prefix.length);
  }
  return null;
}

/// Reads the GIO metadata store with `gio info`; never under `flutter test`, where tests pass
/// a fake.
String? readGioFolderStyle(String path) {
  if (Platform.environment.containsKey('FLUTTER_TEST')) return null;
  try {
    final r = Process.runSync('gio', ['info', '-a', folderStyleMetadata, path]);
    return r.exitCode == 0 ? parseGioInfo('${r.stdout}') : null;
  } on ProcessException {
    return null;
  }
}

/// An attribute read: its value, and whether the file system has no user attributes at all.
typedef AttributeRead = ({String? value, bool unsupported});

/// What a folder icon shows: the symbol and the tint (`#rrggbb` or a colour name).
typedef FolderLook = ({String? symbol, String? tint});

/// Folder looks, cached per folder.
class FolderStyles {
  FolderStyles({
    this.readAttribute = readFolderAttribute,
    this.readMetadata = readGioFolderStyle,
  });

  final AttributeRead Function(String path, String name) readAttribute;
  final String? Function(String path) readMetadata;

  final Map<String, FolderLook> _looks = {};

  FolderLook look(String path) {
    final hit = _looks[path];
    if (hit != null) return hit;
    final custom = readAttribute(path, folderStyleAttribute);
    final style = FolderStyle.parse(
      custom.unsupported ? readMetadata(path) : custom.value,
    );
    var tint = style?.tint;
    if (tint == null) {
      final tags = readAttribute(path, folderTagsAttribute).value ?? '';
      for (final tag in tags.split(',')) {
        final colour = tagColourName(tag.trim());
        if (colour != null) {
          tint = colour;
          break;
        }
      }
    }
    if (_looks.length > 2000) _looks.clear();
    return _looks[path] = (symbol: style?.symbol, tint: tint);
  }

  /// Forgets [paths] (a watch reported a change there).
  void invalidate(Iterable<String> paths) {
    for (final p in paths) {
      _looks.remove(p);
    }
  }
}

/// The looks of the whole shell.
FolderStyles folderStyles = FolderStyles();

/// The icon of the folder at [path] with its colour and symbol: a custom symbol replaces a
/// special folder's own; one silica-icons does not have keeps the special one.
FolderIcon? styledFolderIcon(
  String path, {
  Map<String, String>? env,
  FolderStyles? styles,
}) {
  final look = (styles ?? folderStyles).look(path);
  final folder = resolveFolderIcon(
    path,
    symbol: look.symbol,
    tint: look.tint,
    env: env,
  );
  if (look.symbol != null && folder != null && folder.symbol == null) {
    return resolveFolderIcon(path, tint: look.tint, env: env);
  }
  return folder;
}

typedef _GetC = IntPtr Function(
  Pointer<Utf8>,
  Pointer<Utf8>,
  Pointer<Uint8>,
  IntPtr,
);
typedef _GetDart = int Function(
  Pointer<Utf8>,
  Pointer<Utf8>,
  Pointer<Uint8>,
  int,
);
typedef _ErrnoC = Pointer<Int32> Function();

/// `ENOTSUP` (= `EOPNOTSUPP`) on Linux.
const int _notSupported = 95;

_GetDart? _getxattr;
Pointer<Int32> Function()? _errno;
bool _looked = false;

/// Reads an extended attribute (UTF-8) through libc; a missing attribute, a missing file or a
/// libc without xattr give null, never an exception.
AttributeRead readFolderAttribute(String path, String name) {
  if (!_looked) {
    _looked = true;
    try {
      final lib = DynamicLibrary.process();
      _getxattr = lib.lookupFunction<_GetC, _GetDart>('getxattr');
      if (lib.providesSymbol('__errno_location')) {
        _errno = lib.lookupFunction<_ErrnoC, _ErrnoC>('__errno_location');
      }
    } on ArgumentError {
      _getxattr = null;
    }
  }
  final get = _getxattr;
  if (get == null) return (value: null, unsupported: true);
  bool unsupported() {
    final e = _errno;
    return e != null && e().value == _notSupported;
  }

  final p = path.toNativeUtf8();
  final n = name.toNativeUtf8();
  try {
    final size = get(p, n, nullptr, 0);
    if (size < 0) return (value: null, unsupported: unsupported());
    if (size == 0 || size > 1 << 16) return (value: null, unsupported: false);
    final buffer = calloc<Uint8>(size);
    try {
      final read = get(p, n, buffer, size);
      if (read < 0) return (value: null, unsupported: unsupported());
      return (
        value: utf8.decode(buffer.asTypedList(read), allowMalformed: true),
        unsupported: false,
      );
    } finally {
      calloc.free(buffer);
    }
  } finally {
    calloc
      ..free(p)
      ..free(n);
  }
}
