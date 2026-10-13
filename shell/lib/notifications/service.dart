// Le notifiche da silica-daemon sul bus di sessione (silica/dbus): lo storico all'avvio,
// poi i segnali Added e Removed. Le azioni dell'utente tornano al daemon.

import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:flutter/services.dart';

import '../system/dbus.dart';
import 'model.dart';

const String daemonName = 'dev.frascella.Silica';
const String daemonPath = '/dev/frascella/Silica/Notifications1';
const String daemonInterface = 'dev.frascella.Silica.Notifications1';

/// Lo stato delle notifiche per la shell.
abstract class NotificationsSource implements Listenable {
  /// Lo storico, per id.
  Map<int, ShellNotification> get notifications;

  /// Le notifiche nuove o sostituite, una volta sola: diventano banner.
  Stream<ShellNotification> get posted;

  Future<void> dismiss(int id);
  Future<void> invoke(int id, String action);
  Future<void> clear();
}

/// [NotificationsSource] su D-Bus.
class DBusNotifications extends ChangeNotifier implements NotificationsSource {
  DBusNotifications({DBus? dbus}) : _dbus = dbus ?? DBus.instance {
    _start();
  }

  final DBus _dbus;
  final Map<int, ShellNotification> _notifications = {};
  final StreamController<ShellNotification> _posted =
      StreamController.broadcast();
  StreamSubscription<DBusSignal>? _signals;

  @override
  Map<int, ShellNotification> get notifications =>
      Map.unmodifiable(_notifications);

  @override
  Stream<ShellNotification> get posted => _posted.stream;

  Future<void> _start() async {
    _signals = _dbus
        .signals(path: daemonPath, interface: daemonInterface)
        .listen(
          _signal,
          onError: (Object error) {
            debugPrint('notifiche: segnali non disponibili: $error');
          },
        );
    try {
      final reply = await _call('List');
      final list = reply.isEmpty ? null : reply.first;
      if (list is List) {
        for (final item in list) {
          final n = ShellNotification.fromDBus(item);
          if (n != null) _notifications[n.id] = n;
        }
        notifyListeners();
      }
    } on Object catch (error) {
      // Senza daemon (fuori da Silica, o bus senza silica-daemon) il Centro resta vuoto.
      debugPrint('notifiche: silica-daemon non risponde: $error');
    }
  }

  void _signal(DBusSignal signal) {
    switch (signal.member) {
      case 'Added':
        final n = signal.args.isEmpty
            ? null
            : ShellNotification.fromDBus(signal.args.first);
        if (n == null) return;
        _notifications[n.id] = n;
        _posted.add(n);
        notifyListeners();
      case 'Removed':
        final id = signal.args.isEmpty ? null : signal.args.first;
        if (id is int && _notifications.remove(id) != null) notifyListeners();
    }
  }

  Future<List<Object?>> _call(
    String method, [
    String signature = '',
    List<Object?> args = const [],
  ]) => _dbus.call(
    destination: daemonName,
    path: daemonPath,
    interface: daemonInterface,
    method: method,
    signature: signature,
    args: args,
  );

  Future<void> _quiet(Future<Object?> call) async {
    try {
      await call;
    } on PlatformException catch (error) {
      debugPrint('notifiche: ${error.code}: ${error.message}');
    } on MissingPluginException {
      // Fuori da Silica.
    }
  }

  @override
  Future<void> dismiss(int id) => _quiet(_call('Dismiss', 'u', [id]));

  @override
  Future<void> invoke(int id, String action) =>
      _quiet(_call('InvokeAction', 'us', [id, action]));

  @override
  Future<void> clear() => _quiet(_call('Clear'));

  @override
  void dispose() {
    _signals?.cancel();
    _posted.close();
    super.dispose();
  }
}
