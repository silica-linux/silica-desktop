// I test misurano con Inter vero (il font dell'interfaccia, da silica_ui), non con il font di
// prova: larghezze e troncamenti sono quelli della finestra.

import 'dart:async';

import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';

Future<void> testExecutable(FutureOr<void> Function() testMain) async {
  TestWidgetsFlutterBinding.ensureInitialized();
  final inter = FontLoader('packages/silica_ui/Inter')
    ..addFont(rootBundle.load('packages/silica_ui/fonts/Inter-Regular.ttf'))
    ..addFont(rootBundle.load('packages/silica_ui/fonts/Inter-SemiBold.ttf'));
  await inter.load();
  await testMain();
}
