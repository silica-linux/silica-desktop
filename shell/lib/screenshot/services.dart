// What the screenshot tools ask of the system: Ossido's capture commands, launching apps,
// silica-daemon's interface sounds and clipboard, the drag and drop of the thumbnail (the
// embedder's `silica/drag`) and the folder chooser (Mustache's dialog). Tests replace it.

import 'dart:async';
import 'dart:convert';
import 'dart:io';
import 'dart:ui' as ui;

import 'package:flutter/services.dart';
import 'package:silica_ui/silica_ui.dart' show SilicaDrag, SilicaDragFinished;

import '../dock/dock.dart' show shellQuote;
import '../system/dbus.dart';
import '../system/desktop_entries.dart';
import '../system/shell_channel.dart';
import '../system/sounds.dart';

/// The sound of a capture (data/sounds/screen-capture.wav, tools/sounds/synth.py).
const String captureSound = UiSound.screenCapture;

abstract class ScreenshotServices {
  /// Runs an Ossido command; true if it ran.
  Future<bool> execute(String id, [Map<String, Object?>? params]);

  /// Starts a command line (an app).
  void launch(String command);

  /// Plays a named interface sound (silica-daemon `Sound1.PlaySound`).
  void playSound(String name);

  /// Puts a PNG file on the clipboard (silica-daemon `Clipboard1.CopyImage`).
  Future<bool> copyImage(String path);

  /// Starts a drag of the file `path` out of the layer, with `icon` grabbed at `hotspot`
  /// (icon pixels). True if Ossido took it.
  Future<bool> startDrag(String path, {ui.Image? icon, ui.Offset? hotspot});

  /// The drags that left the layer finish: `copy`, `move` or `none`.
  Stream<String> get dragFinished;

  /// Asks for a folder; null if cancelled or impossible.
  Future<String?> chooseFolder({required String title, String? start});
}

/// The real services.
class SystemScreenshotServices implements ScreenshotServices {
  SystemScreenshotServices({this.entries = const {}});

  /// The installed apps (the file manager runs the folder chooser).
  final Map<String, DesktopEntry> entries;

  @override
  Future<bool> execute(String id, [Map<String, Object?>? params]) =>
      ShellChannel.instance.execute(id, params);

  @override
  void launch(String command) => ShellChannel.instance.launch(command);

  @override
  void playSound(String name) => playUiSound(name).ignore();

  @override
  Future<bool> copyImage(String path) async {
    try {
      final reply = await DBus.instance.call(
        destination: 'dev.frascella.Silica',
        path: '/dev/frascella/Silica/Clipboard1',
        interface: 'dev.frascella.Silica.Clipboard1',
        method: 'CopyImage',
        signature: 's',
        args: [path],
      );
      return reply.firstOrNull == true;
    } on PlatformException {
      return false;
    } on MissingPluginException {
      return false;
    }
  }

  @override
  Future<bool> startDrag(
    String path, {
    ui.Image? icon,
    ui.Offset? hotspot,
  }) async {
    final args = <String, Object?>{
      'uris': [Uri.file(path).toString()],
    };
    if (icon != null) {
      final bytes = await icon.toByteData(format: ui.ImageByteFormat.rawRgba);
      if (bytes != null) {
        args['icon'] = {
          'width': icon.width,
          'height': icon.height,
          'rgba': base64Encode(bytes.buffer.asUint8List()),
          // The point of the image that stays under the pointer.
          if (hotspot != null) ...{
            'x': hotspot.dx.round(),
            'y': hotspot.dy.round(),
          },
        };
      }
    }
    try {
      return await SilicaDrag.channel.invokeMethod<bool>('start', args) ??
          false;
    } on MissingPluginException {
      return false;
    } on PlatformException {
      return false;
    }
  }

  @override
  Stream<String> get dragFinished => SilicaDrag.events
      .where((e) => e is SilicaDragFinished)
      .map((e) => (e as SilicaDragFinished).action);

  @override
  Future<String?> chooseFolder({required String title, String? start}) async {
    final files = entries[fileManagerId];
    if (files == null) return null;
    String arg(String value) => '--dart-arg ${shellQuote(value)}';
    final command = [
      files.exec,
      arg('--dialog=open'),
      arg('--directory'),
      arg('--title=$title'),
      if (start != null) arg('--folder=$start'),
    ].join(' ');
    try {
      final result = await Process.run('sh', ['-c', command]);
      if (result.exitCode != 0) return null;
      for (final line in '${result.stdout}'.split('\n')) {
        final uri = Uri.tryParse(line.trim());
        if (uri != null && uri.scheme == 'file') return uri.toFilePath();
      }
    } on ProcessException {
      return null;
    }
    return null;
  }
}
