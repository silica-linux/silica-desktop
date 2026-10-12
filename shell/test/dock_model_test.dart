// I numeri del Dock contro plan/reference/dock.md.

import 'package:flutter_test/flutter_test.dart';
import 'package:silica_shell/dock/model.dart';

/// Il Dock delle riprese: 12 app, separatore, cartella e cestino, su 2560 pt.
final items = [
  for (var i = 0; i < 12; i++) Item.app('App $i'),
  const Item.separator(),
  const Item.app('Download'),
  const Item.app('Cestino'),
];
const width = 2560.0;
const height = 200.0;

void main() {
  test('a riposo: celle, fondo e icone come nelle riprese', () {
    final l = layout(items, width: width, height: height);
    // Celle: 14 × 63 + 28,35 = 910,35 pt, centrate.
    final cellsWidth = l.cells.last.$1 + l.cells.last.$2 - l.cells.first.$1;
    expect(cellsWidth, closeTo(910.35, 0.01));
    // Fondo: largo ~927 pt (x 816–1743), alto 79, a 5 pt dal bordo.
    final (bx, by, bw, bh) = l.background;
    expect(bw, closeTo(927, 1.5));
    expect(bx + bw / 2, closeTo(width / 2, 0.01));
    expect(bh, 79);
    expect(height - (by + bh), 5);
    // Icona: 48 pt, 16 pt sotto la cima del fondo.
    final (_, iy, side) = iconRect(l, 0, height);
    expect(side, 48);
    expect(iy - by, 16);
  });

  test('ingrandimento: 68 pt sotto il puntatore, profilo della tabella', () {
    final rest = layout(items, width: width, height: height);
    final mail = rest.center(4);
    final l = layout(items, width: width, height: height, pointerX: mail, amount: 1);
    expect(l.cells[4].$2, closeTo(68, 0.01));
    // A 2 celle (126 pt) dal puntatore f ≈ 0,499: 63 · (1 + 0,0794 · 0,499).
    expect(l.cells[6].$2, closeTo(63 * (1 + (68 / 63 - 1) * profile(126)), 0.01));
    // Il punto sotto il puntatore resta fermo.
    expect(l.center(4), closeTo(mail, 0.01));
    // Ogni lato cresce al massimo di ~9,66 pt (lista da 910,35 a 929,67).
    final grown = l.cells.last.$1 + l.cells.last.$2 - l.cells.first.$1;
    expect(grown, closeTo(929.67, 0.6));
    // Il fondo non si alza.
    expect(l.background.$2, rest.background.$2);
  });

  test('vicino a un estremo cresce solo l\'altro lato', () {
    // Come x 820 nelle riprese: nel margine del fondo, prima della prima cella.
    final rest = layout(items, width: width, height: height);
    final edge = rest.background.$1 + 3.5;
    final l = layout(items, width: width, height: height, pointerX: edge, amount: 1);
    expect(l.cells.first.$1, closeTo(rest.cells.first.$1, 0.3));
  });

  test('ingrandimento massimo: 132 pt e il bordo dell\'icona sale di ~7 pt', () {
    final rest = layout(items, width: width, height: height);
    final mail = rest.center(4);
    final l = layout(items,
        width: width, height: height, pointerX: mail, amount: 1, magnification: maxMagnification);
    expect(l.cells[4].$2, closeTo(132, 0.01));
    final (_, restTop, restSide) = iconRect(rest, 4, height);
    final (_, top, side) = iconRect(l, 4, height);
    expect((restTop + restSide) - (top + side), closeTo(7, 1));
  });

  test('entrata: ~60 ms a 68 pt, ~160 ms a 132 pt, curva misurata', () {
    expect(enterDuration(defaultMagnification).inMilliseconds, 60);
    expect(enterDuration(maxMagnification).inMilliseconds, 160);
    expect(enterProgress(0), 0);
    expect(enterProgress(50 / 160), closeTo(0.2065, 0.001));
    expect(enterProgress(1), 1);
  });

  test('zona attiva: 5 pt sopra il fondo, dentro le celle', () {
    final rest = layout(items, width: width, height: height);
    final top = rest.background.$2;
    final x = rest.center(3);
    expect(inActiveZone(rest, x, top - 5, height), isTrue);
    expect(inActiveZone(rest, x, top - 6, height), isFalse);
    expect(inActiveZone(rest, rest.background.$1 - 1, top + 10, height), isFalse);
    expect(inActiveZone(rest, rest.background.$1 + 3.5, top + 10, height), isTrue);
  });
}
