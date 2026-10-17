import 'dart:io';

import 'package:flutter_test/flutter_test.dart';

void main() {
  // silica-ui#93: one scroll bar everywhere. The shell draws AmalfiScrollbar; no Material or
  // raw scroll bars and no local copies.
  test('the shell uses no platform scroll bar', () {
    final bad = RegExp(r'\b(RawScrollbar|CupertinoScrollbar|Scrollbar)\s*\(');
    final hits = [
      for (final f in Directory('lib').listSync(recursive: true))
        if (f is File &&
            f.path.endsWith('.dart') &&
            bad.hasMatch(f.readAsStringSync()))
          f.path,
    ];
    expect(hits, isEmpty);
  });
}
