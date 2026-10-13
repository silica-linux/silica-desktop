// Il canale silica/dbus dell'embedder: chiamate D-Bus e segnali con valori JSON
// (crates/embedder/src/dbus). I valori vanno con la firma degli argomenti; le risposte
// arrivano come liste di argomenti (strutture come liste, dizionari come mappe).

import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:flutter/services.dart';

/// Un bus D-Bus.
enum DBusBus { session, system }

/// Un segnale ricevuto.
@immutable
class DBusSignal {
  const DBusSignal({
    required this.sender,
    required this.path,
    required this.interface,
    required this.member,
    required this.args,
  });

  final String sender;
  final String path;
  final String interface;
  final String member;
  final List<Object?> args;
}

class DBus {
  DBus._() {
    channel.setMethodCallHandler((call) async {
      if (call.method == 'signal') _dispatch(call.arguments);
      return null;
    });
  }

  static final DBus instance = DBus._();

  static const MethodChannel channel = MethodChannel(
    'silica/dbus',
    JSONMethodCodec(),
  );

  final Map<int, StreamController<DBusSignal>> _subscriptions = {};

  /// Chiama un metodo; restituisce gli argomenti della risposta. Un errore D-Bus è una
  /// [PlatformException] con il nome dell'errore come `code`.
  Future<List<Object?>> call({
    DBusBus bus = DBusBus.session,
    required String destination,
    required String path,
    String? interface,
    required String method,
    String signature = '',
    List<Object?> args = const [],
  }) async {
    final reply = await channel.invokeMethod<List<Object?>>('call', {
      'bus': bus.name,
      'destination': destination,
      'path': path,
      'interface': ?interface,
      'method': method,
      'signature': signature,
      'args': args,
    });
    return reply ?? const [];
  }

  /// Una proprietà (`org.freedesktop.DBus.Properties.Get`).
  Future<Object?> getProperty({
    DBusBus bus = DBusBus.session,
    required String destination,
    required String path,
    required String interface,
    required String name,
  }) async {
    final reply = await call(
      bus: bus,
      destination: destination,
      path: path,
      interface: 'org.freedesktop.DBus.Properties',
      method: 'Get',
      signature: 'ss',
      args: [interface, name],
    );
    return reply.isEmpty ? null : reply.first;
  }

  /// Tutte le proprietà di un'interfaccia (`org.freedesktop.DBus.Properties.GetAll`).
  Future<Map<String, Object?>> getAll({
    DBusBus bus = DBusBus.session,
    required String destination,
    required String path,
    required String interface,
  }) async {
    final reply = await call(
      bus: bus,
      destination: destination,
      path: path,
      interface: 'org.freedesktop.DBus.Properties',
      method: 'GetAll',
      signature: 's',
      args: [interface],
    );
    final first = reply.isEmpty ? null : reply.first;
    return first is Map ? first.cast<String, Object?>() : const {};
  }

  /// I segnali che corrispondono ai filtri. L'iscrizione sul bus si chiude quando
  /// l'ultimo ascoltatore cancella la sottoscrizione dello stream.
  Stream<DBusSignal> signals({
    DBusBus bus = DBusBus.session,
    String? sender,
    String? path,
    String? interface,
    String? member,
  }) {
    int? id;
    late final StreamController<DBusSignal> controller;
    controller = StreamController<DBusSignal>(
      onListen: () async {
        try {
          id = await channel.invokeMethod<int>('subscribe', {
            'bus': bus.name,
            'sender': ?sender,
            'path': ?path,
            'interface': ?interface,
            'member': ?member,
          });
        } on PlatformException catch (error, stack) {
          controller.addError(error, stack);
          await controller.close();
          return;
        } on MissingPluginException catch (error, stack) {
          // Fuori da Silica (test, altro embedder): nessun segnale.
          controller.addError(error, stack);
          await controller.close();
          return;
        }
        final subscribed = id;
        if (subscribed == null) return;
        if (controller.isClosed) {
          await channel.invokeMethod<bool>('unsubscribe', {'id': subscribed});
          return;
        }
        _subscriptions[subscribed] = controller;
      },
      onCancel: () async {
        final subscribed = id;
        if (subscribed == null) return;
        _subscriptions.remove(subscribed);
        try {
          await channel.invokeMethod<bool>('unsubscribe', {'id': subscribed});
        } on PlatformException {
          // L'iscrizione è già chiusa.
        }
      },
    );
    return controller.stream;
  }

  void _dispatch(Object? arguments) {
    if (arguments is! Map) return;
    final id = arguments['id'];
    final controller = id is int ? _subscriptions[id] : null;
    if (controller == null || controller.isClosed) return;
    final args = arguments['args'];
    controller.add(
      DBusSignal(
        sender: '${arguments['sender'] ?? ''}',
        path: '${arguments['path'] ?? ''}',
        interface: '${arguments['interface'] ?? ''}',
        member: '${arguments['member'] ?? ''}',
        args: args is List ? args : const [],
      ),
    );
  }
}
