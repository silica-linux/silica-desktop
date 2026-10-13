// Lo stato del sistema da silica-daemon (dev.frascella.Silica.System1, docs/daemon.md):
// Wi-Fi, Bluetooth, batteria, luminosità, volume, Non disturbare. Lo usano il Centro di
// controllo, la barra dei menu e le notifiche (Non disturbare).

import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:flutter/services.dart';

import 'dbus.dart';

const String systemName = 'dev.frascella.Silica';
const String systemPath = '/dev/frascella/Silica/System1';
const String systemInterface = 'dev.frascella.Silica.System1';

/// Una fotografia dello stato. Le sorgenti assenti hanno `available` falso.
@immutable
class SystemSnapshot {
  const SystemSnapshot({
    this.wifiAvailable = false,
    this.wifiEnabled = false,
    this.wifiSsid = '',
    this.wifiStrength = 0,
    this.wired = false,
    this.bluetoothAvailable = false,
    this.bluetoothPowered = false,
    this.batteryPresent = false,
    this.batteryPercent = 0,
    this.batteryCharging = false,
    this.brightnessAvailable = false,
    this.brightness = 0,
    this.audioAvailable = false,
    this.volume = 0,
    this.muted = false,
    this.audioName = '',
    this.doNotDisturb = false,
  });

  final bool wifiAvailable;
  final bool wifiEnabled;
  final String wifiSsid;
  final int wifiStrength;
  final bool wired;
  final bool bluetoothAvailable;
  final bool bluetoothPowered;
  final bool batteryPresent;
  final double batteryPercent;
  final bool batteryCharging;
  final bool brightnessAvailable;
  final double brightness;
  final bool audioAvailable;
  final double volume;
  final bool muted;

  /// Il nome dell'uscita audio predefinita (titolo dell'HUD del volume).
  final String audioName;
  final bool doNotDisturb;

  static SystemSnapshot fromDict(Map<Object?, Object?> d) {
    bool flag(String key) => d[key] == true;
    double number(String key) {
      final value = d[key];
      return value is num && value.isFinite ? value.toDouble() : 0;
    }

    return SystemSnapshot(
      wifiAvailable: flag('wifi.available'),
      wifiEnabled: flag('wifi.enabled'),
      wifiSsid: d['wifi.ssid'] is String ? d['wifi.ssid'] as String : '',
      wifiStrength: number('wifi.strength').round().clamp(0, 100),
      wired: flag('wifi.wired'),
      bluetoothAvailable: flag('bluetooth.available'),
      bluetoothPowered: flag('bluetooth.powered'),
      batteryPresent: flag('battery.present'),
      batteryPercent: number('battery.percent').clamp(0, 100),
      batteryCharging: flag('battery.charging'),
      brightnessAvailable: flag('brightness.available'),
      brightness: number('brightness').clamp(0, 1),
      audioAvailable: flag('audio.available'),
      volume: number('audio.volume').clamp(0, 1),
      muted: flag('audio.muted'),
      audioName: d['audio.name'] is String ? d['audio.name'] as String : '',
      doNotDisturb: flag('dnd'),
    );
  }

  SystemSnapshot copyWith({
    bool? wifiEnabled,
    bool? bluetoothPowered,
    double? brightness,
    double? volume,
    bool? muted,
    bool? doNotDisturb,
  }) => SystemSnapshot(
    wifiAvailable: wifiAvailable,
    wifiEnabled: wifiEnabled ?? this.wifiEnabled,
    wifiSsid: wifiSsid,
    wifiStrength: wifiStrength,
    wired: wired,
    bluetoothAvailable: bluetoothAvailable,
    bluetoothPowered: bluetoothPowered ?? this.bluetoothPowered,
    batteryPresent: batteryPresent,
    batteryPercent: batteryPercent,
    batteryCharging: batteryCharging,
    brightnessAvailable: brightnessAvailable,
    brightness: brightness ?? this.brightness,
    audioAvailable: audioAvailable,
    volume: volume ?? this.volume,
    muted: muted ?? this.muted,
    audioName: audioName,
    doNotDisturb: doNotDisturb ?? this.doNotDisturb,
  );
}

/// Lo stato e le azioni; le azioni aggiornano subito la fotografia (la risposta del
/// daemon arriva poi con `Changed`).
abstract class SystemSource implements ValueListenable<SystemSnapshot> {
  Future<void> setWifi(bool enabled);
  Future<void> setBluetooth(bool powered);
  Future<void> setBrightness(double value);
  Future<void> setVolume(double value);
  Future<void> setMuted(bool muted);
  Future<void> setDoNotDisturb(bool enabled);
}

/// [SystemSource] su D-Bus.
class DBusSystem extends ValueNotifier<SystemSnapshot> implements SystemSource {
  DBusSystem({DBus? dbus})
    : _dbus = dbus ?? DBus.instance,
      super(const SystemSnapshot()) {
    _start();
  }

  final DBus _dbus;
  StreamSubscription<DBusSignal>? _signals;

  Future<void> _start() async {
    _signals = _dbus
        .signals(path: systemPath, interface: systemInterface)
        .listen(
          (signal) {
            final dict = signal.args.isEmpty ? null : signal.args.first;
            if (signal.member == 'Changed' && dict is Map) {
              value = SystemSnapshot.fromDict(dict);
            }
          },
          onError: (Object error) {
            debugPrint('sistema: segnali non disponibili: $error');
          },
        );
    try {
      final reply = await _call('State');
      final dict = reply.isEmpty ? null : reply.first;
      if (dict is Map) value = SystemSnapshot.fromDict(dict);
    } on Object catch (error) {
      debugPrint('sistema: silica-daemon non risponde: $error');
    }
  }

  Future<List<Object?>> _call(
    String method, [
    String signature = '',
    List<Object?> args = const [],
  ]) => _dbus.call(
    destination: systemName,
    path: systemPath,
    interface: systemInterface,
    method: method,
    signature: signature,
    args: args,
  );

  Future<void> _set(String method, String signature, Object arg) async {
    try {
      await _call(method, signature, [arg]);
    } on PlatformException catch (error) {
      debugPrint('sistema: $method: ${error.code}: ${error.message}');
    } on MissingPluginException {
      // Fuori da Silica.
    }
  }

  @override
  Future<void> setWifi(bool enabled) {
    value = value.copyWith(wifiEnabled: enabled);
    return _set('SetWifiEnabled', 'b', enabled);
  }

  @override
  Future<void> setBluetooth(bool powered) {
    value = value.copyWith(bluetoothPowered: powered);
    return _set('SetBluetoothPowered', 'b', powered);
  }

  @override
  Future<void> setBrightness(double v) {
    value = value.copyWith(brightness: v);
    return _set('SetBrightness', 'd', v);
  }

  @override
  Future<void> setVolume(double v) {
    value = value.copyWith(volume: v);
    return _set('SetVolume', 'd', v);
  }

  @override
  Future<void> setMuted(bool muted) {
    value = value.copyWith(muted: muted);
    return _set('SetMuted', 'b', muted);
  }

  @override
  Future<void> setDoNotDisturb(bool enabled) {
    value = value.copyWith(doNotDisturb: enabled);
    return _set('SetDoNotDisturb', 'b', enabled);
  }

  @override
  void dispose() {
    _signals?.cancel();
    super.dispose();
  }
}
