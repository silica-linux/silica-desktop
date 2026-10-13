// Il canale silica/shell dell'embedder: le finestre aperte (da Ossido) e i comandi
// activate e launch. Fuori da Silica il canale non risponde: niente finestre.

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

  /// L'identificatore della finestra con il focus (vuoto: nessuna).
  final ValueNotifier<String> focused = ValueNotifier('');

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
    } on MissingPluginException {
      // Fuori da Silica.
    }
  }

  Future<bool> activate(String id) => _call('activate', {'id': id});

  Future<bool> launch(String command) => _call('launch', {'command': command});

  Future<bool> _call(String method, Map<String, String> args) async {
    try {
      return await channel.invokeMethod<bool>(method, args) ?? false;
    } on MissingPluginException {
      return false;
    }
  }
}
