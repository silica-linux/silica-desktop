// The system state from silica-daemon (dev.frascella.Silica.System1, docs/daemon.md):
// Wi-Fi, Bluetooth, battery, brightness, volume, Do Not Disturb. Used by the Control
// Center, the menu bar and the notifications (Do Not Disturb).

import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:flutter/services.dart';

import 'dbus.dart';

const String systemName = 'dev.frascella.Silica';
const String systemPath = '/dev/frascella/Silica/System1';
const String systemInterface = 'dev.frascella.Silica.System1';

/// A snapshot of the state. Missing sources have `available` false.
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
    this.brightnessRequests = 0,
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

  /// How many times Silica asked for a brightness (keys, sliders): it changes only for the
  /// user, not for adaptive brightness.
  final int brightnessRequests;
  final bool audioAvailable;
  final double volume;
  final bool muted;

  /// The name of the default audio output (title of the volume HUD).
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
      brightnessRequests: number('brightness.requests').round(),
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
    int? brightnessRequests,
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
    brightnessRequests: brightnessRequests ?? this.brightnessRequests,
    audioAvailable: audioAvailable,
    volume: volume ?? this.volume,
    muted: muted ?? this.muted,
    audioName: audioName,
    doNotDisturb: doNotDisturb ?? this.doNotDisturb,
  );
}

/// A visible wireless network (Wi-Fi detail panel).
@immutable
class WifiNetwork {
  const WifiNetwork({
    required this.ssid,
    required this.strength,
    this.secure = false,
    this.known = false,
    this.active = false,
  });

  final String ssid;

  /// 0–100.
  final int strength;
  final bool secure;

  /// There is a saved connection.
  final bool known;
  final bool active;

  /// From `(sybbb)` of `WifiNetworks`.
  static WifiNetwork? fromTuple(Object? tuple) {
    if (tuple is! List || tuple.length < 5) return null;
    final [ssid, strength, secure, known, active, ...] = tuple;
    if (ssid is! String || strength is! int) return null;
    return WifiNetwork(
      ssid: ssid,
      strength: strength.clamp(0, 100),
      secure: secure == true,
      known: known == true,
      active: active == true,
    );
  }
}

/// A paired Bluetooth device (Bluetooth detail panel).
@immutable
class BluetoothDevice {
  const BluetoothDevice({
    required this.address,
    required this.name,
    this.icon = '',
    this.connected = false,
  });

  final String address;
  final String name;

  /// The freedesktop icon name BlueZ gives (`input-mouse`, `audio-headset`…).
  final String icon;
  final bool connected;

  /// From `(sssb)` of `BluetoothDevices`.
  static BluetoothDevice? fromTuple(Object? tuple) {
    if (tuple is! List || tuple.length < 4) return null;
    final [address, name, icon, connected, ...] = tuple;
    if (address is! String || name is! String) return null;
    return BluetoothDevice(
      address: address,
      name: name,
      icon: icon is String ? icon : '',
      connected: connected == true,
    );
  }
}

/// An audio output (Sound detail panel).
@immutable
class AudioOutput {
  const AudioOutput({
    required this.id,
    required this.name,
    this.isDefault = false,
  });

  final int id;
  final String name;
  final bool isDefault;

  /// From `(usb)` of `AudioOutputs`.
  static AudioOutput? fromTuple(Object? tuple) {
    if (tuple is! List || tuple.length < 3) return null;
    final [id, name, isDefault, ...] = tuple;
    if (id is! int || name is! String) return null;
    return AudioOutput(id: id, name: name, isDefault: isDefault == true);
  }
}

/// The state and the actions; actions update the snapshot right away (the daemon's
/// answer arrives later with `Changed`).
abstract class SystemSource implements ValueListenable<SystemSnapshot> {
  /// The visible networks, the one in use first (empty if the daemon does not answer).
  Future<List<WifiNetwork>> wifiNetworks();
  Future<void> connectWifi(String ssid);
  Future<List<AudioOutput>> audioOutputs();
  Future<void> setAudioOutput(int id);

  /// The paired Bluetooth devices, connected first.
  Future<List<BluetoothDevice>> bluetoothDevices();
  Future<void> connectBluetooth(String address, bool connect);

  Future<void> setWifi(bool enabled);
  Future<void> setBluetooth(bool powered);
  Future<void> setBrightness(double value);
  Future<void> setVolume(double value);
  Future<void> setMuted(bool muted);
  Future<void> setDoNotDisturb(bool enabled);
}

/// [SystemSource] over D-Bus.
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
            debugPrint('system: signals not available: $error');
          },
        );
    try {
      final reply = await _call('State');
      final dict = reply.isEmpty ? null : reply.first;
      if (dict is Map) value = SystemSnapshot.fromDict(dict);
    } on Object catch (error) {
      debugPrint('system: silica-daemon does not answer: $error');
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
      debugPrint('system: $method: ${error.code}: ${error.message}');
    } on MissingPluginException {
      // Outside Silica.
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

  Future<List<T>> _list<T>(String method, T? Function(Object?) parse) async {
    try {
      final reply = await _call(method);
      final list = reply.isEmpty ? null : reply.first;
      return [
        if (list is List)
          for (final item in list)
            if (parse(item) case final T value) value,
      ];
    } on PlatformException catch (error) {
      debugPrint('system: $method: ${error.code}: ${error.message}');
    } on MissingPluginException {
      // Outside Silica.
    }
    return const [];
  }

  @override
  Future<List<WifiNetwork>> wifiNetworks() =>
      _list('WifiNetworks', WifiNetwork.fromTuple);

  @override
  Future<void> connectWifi(String ssid) => _set('ConnectWifi', 's', ssid);

  @override
  Future<List<AudioOutput>> audioOutputs() =>
      _list('AudioOutputs', AudioOutput.fromTuple);

  @override
  Future<List<BluetoothDevice>> bluetoothDevices() =>
      _list('BluetoothDevices', BluetoothDevice.fromTuple);

  @override
  Future<void> connectBluetooth(String address, bool connect) async {
    try {
      await _call('ConnectBluetooth', 'sb', [address, connect]);
    } on PlatformException catch (error) {
      debugPrint('system: ConnectBluetooth: ${error.code}: ${error.message}');
    } on MissingPluginException {
      // Outside Silica.
    }
  }

  @override
  Future<void> setAudioOutput(int id) => _set('SetAudioOutput', 'u', id);

  @override
  void dispose() {
    _signals?.cancel();
    super.dispose();
  }
}
