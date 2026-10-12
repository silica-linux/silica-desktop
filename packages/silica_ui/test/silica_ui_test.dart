import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:silica_ui/silica_ui.dart';

void main() {
  test('il testo usa Inter dal pacchetto', () {
    final style = uiText(size: 13, color: const Color(0xFFFFFFFF));
    expect(style.fontFamily, 'packages/silica_ui/Inter');
    expect(style.fontSize, 13);
  });
}
