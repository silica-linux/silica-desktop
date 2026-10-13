// Il canale silica/shell dell'embedder: le finestre aperte (da Ossido) e i comandi
// activate e launch. Fuori da Silica il canale non risponde: niente finestre.

import 'dart:async';
import 'dart:io';
import 'dart:ui' as ui;

import 'package:flutter/foundation.dart';
import 'package:flutter/services.dart';

/// Una finestra aperta.
@immutable
class Toplevel {
  const Toplevel({required this.id, required this.title, required this.appId});

  final String id;
  final String title;
  final String appId;

  static Toplevel fromJson(Object? json) {
    final map = json is Map ? json : const {};
    return Toplevel(
      id: '${map['id'] ?? ''}',
      title: '${map['title'] ?? ''}',
      appId: '${map['app_id'] ?? ''}',
    );
  }
}

class ShellChannel {
  ShellChannel._() {
    channel.setMethodCallHandler((call) async {
      switch (call.method) {
        case 'toplevels':
          _update(call.arguments);
        case 'focused':
          focused.value = '${call.arguments ?? ''}';
        case 'command':
          _commands.add('${call.arguments ?? ''}');
        case 'minimized':
          final list = call.arguments;
          minimized.value = [
            if (list is List)
              for (final id in list) '$id',
          ];
        case 'thumbnail':
          _thumbnailArrived(call.arguments);
      }
      return null;
    });
  }

  static final ShellChannel instance = ShellChannel._();

  static const MethodChannel channel = MethodChannel(
    'silica/shell',
    JSONMethodCodec(),
  );

  /// Le finestre aperte, aggiornate a ogni cambiamento.
  final ValueNotifier<List<Toplevel>> toplevels = ValueNotifier(const []);

  final StreamController<String> _commands = StreamController.broadcast();

  /// I comandi che Ossido inoltra alla shell (es. `faro.toggle`).
  Stream<String> get commands => _commands.stream;

  /// L'identificatore della finestra con il focus (vuoto: nessuna).
  final ValueNotifier<String> focused = ValueNotifier('');

  /// Le finestre ridotte, dalla prima ridotta (Ossido, silica-shell-v1 versione 2).
  final ValueNotifier<List<String>> minimized = ValueNotifier(const []);

  final Map<String, Completer<ui.Image?>> _thumbnails = {};

  /// La miniatura di una finestra ridotta (null se non c'è).
  Future<ui.Image?> thumbnail(String id) {
    final pending = _thumbnails[id];
    if (pending != null) return pending.future;
    final completer = Completer<ui.Image?>();
    _thumbnails[id] = completer;
    _call('thumbnail', {'id': id}).then((sent) {
      if (!sent && !completer.isCompleted) {
        _thumbnails.remove(id);
        completer.complete(null);
      }
    });
    return completer.future;
  }

  Future<void> _thumbnailArrived(Object? args) async {
    final map = args is Map ? args : const {};
    final id = '${map['id'] ?? ''}';
    final completer = _thumbnails.remove(id);
    if (completer == null) return;
    final width = map['width'] is int ? map['width'] as int : 0;
    final height = map['height'] is int ? map['height'] as int : 0;
    final path = '${map['path'] ?? ''}';
    if (width <= 0 || height <= 0 || path.isEmpty) {
      completer.complete(null);
      return;
    }
    try {
      final file = File(path);
      final bytes = await file.readAsBytes();
      await file.delete();
      if (bytes.length < width * height * 4) {
        completer.complete(null);
        return;
      }
      ui.decodeImageFromPixels(
        bytes,
        width,
        height,
        ui.PixelFormat.rgba8888,
        completer.complete,
      );
    } on FileSystemException {
      completer.complete(null);
    }
  }

  /// La finestra con il focus, se è nell'elenco.
  Toplevel? get focusedToplevel =>
      toplevels.value.where((t) => t.id == focused.value).firstOrNull;

  void _update(Object? list) {
    toplevels.value = [
      if (list is List)
        for (final item in list) Toplevel.fromJson(item),
    ];
  }

  /// Legge l'elenco attuale (all'avvio, per le finestre annunciate prima di Dart).
  Future<void> refresh() async {
    try {
      _update(await channel.invokeMethod<Object?>('list'));
      focused.value = await channel.invokeMethod<String>('focused') ?? '';
      final list = await channel.invokeMethod<Object?>('minimized');
      minimized.value = [
        if (list is List)
          for (final id in list) '$id',
      ];
    } on MissingPluginException {
      // Fuori da Silica.
    }
  }

  Future<bool> activate(String id) => _call('activate', {'id': id});

  Future<bool> launch(String command) => _call('launch', {'command': command});

  /// Esegue un comando del catalogo di Ossido (es. `faro.toggle` dalla barra).
  Future<bool> execute(String id) => _call('execute', {'id': id});

  Future<bool> _call(String method, Map<String, String> args) async {
    try {
      return await channel.invokeMethod<bool>(method, args) ?? false;
    } on MissingPluginException {
      return false;
    }
  }
}
