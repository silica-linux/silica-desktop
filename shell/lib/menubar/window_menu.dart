// The window arrangement popover (windows.md, long press on the green button;
// measured in silica-reference, BACKLOG-1 / W-08): a menu with its tip pointing up at the
// button, 242 wide (229 tall with the "Move to" section), two rows of four cells of 54 × 44 holding 25 × 20 icons
// (the pointer's highlight is 46 × 36) below their headings, then the full screen item.
// The items run Ossido's `window.tile_*` / `window.arrange_*` commands on the focused window
// (the one of the button). Esc closes it; moving the pointer away does not. The delay (hover
// ~813 ms, press ~628 ms) and the animation of the frames are Ossido's.

import 'package:flutter/widgets.dart';
import 'package:silica_ui/silica_ui.dart';

import '../l10n/l10n.dart';

const double windowMenuWidth = 242;

/// The panel's height with one display. The measured 229 includes the "Move to <display>"
/// section (separator and item, 37 pt), which only exists with several displays; Ossido
/// drives one for now.
const double windowMenuHeight = 192;
const double windowMenuHeightWithDisplays = 229;

/// A cell of the icon rows: 54 × 44, the icon (25 × 20) centered in it.
const Size windowMenuCell = Size(54, 44);
const double windowMenuRow = 44;
const Size windowMenuIcon = Size(25, 20);
const double windowMenuPitch = 54;

/// The highlight under the pointer.
const Size windowMenuHover = Size(46, 36);

/// Left margin of the first cell: four cells and the margins fill the panel's width.
const double windowMenuMargin = (windowMenuWidth - 4 * windowMenuPitch) / 2;

/// The parts of an icon: rectangles as fractions of the box; the first is the chosen window
/// (dark), the others the other windows (grey).
typedef TileGlyph = List<Rect>;

const Map<String, TileGlyph> tileGlyphs = {
  'window.tile_left': [Rect.fromLTWH(0, 0, 0.5, 1)],
  'window.tile_right': [Rect.fromLTWH(0.5, 0, 0.5, 1)],
  'window.tile_top': [Rect.fromLTWH(0, 0, 1, 0.5)],
  'window.tile_bottom': [Rect.fromLTWH(0, 0.5, 1, 0.5)],
  'window.tile_fill': [Rect.fromLTWH(0, 0, 1, 1)],
  'window.arrange_2': [
    Rect.fromLTWH(0, 0, 0.5, 1),
    Rect.fromLTWH(0.5, 0, 0.5, 1),
  ],
  'window.arrange_3': [
    Rect.fromLTWH(0, 0, 0.5, 1),
    Rect.fromLTWH(0.5, 0, 0.5, 0.5),
    Rect.fromLTWH(0.5, 0.5, 0.5, 0.5),
  ],
  'window.arrange_4': [
    Rect.fromLTWH(0, 0, 0.5, 0.5),
    Rect.fromLTWH(0.5, 0, 0.5, 0.5),
    Rect.fromLTWH(0, 0.5, 0.5, 0.5),
    Rect.fromLTWH(0.5, 0.5, 0.5, 0.5),
  ],
};

/// The popover items; `execute` runs a command of Ossido's catalog.
List<MenuEntry> windowMenuEntries(
  ValueChanged<String> execute, {
  AppLocalizations? texts,
}) => [
  MenuSection((texts ?? englishTexts).halfScreen),
  _row(const [
    'window.tile_left',
    'window.tile_right',
    'window.tile_top',
    'window.tile_bottom',
  ], execute),
  const MenuDivider(),
  MenuSection((texts ?? englishTexts).arrange),
  _row(const [
    'window.tile_fill',
    'window.arrange_2',
    'window.arrange_3',
    'window.arrange_4',
  ], execute),
  const MenuDivider(),
  MenuAction(
    (texts ?? englishTexts).fullScreen,
    icon: (color) => SizedBox(
      width: 14,
      height: 11,
      child: CustomPaint(
        painter: TilePainter(
          const [Rect.fromLTWH(0, 0, 1, 1)],
          color,
          stroke: 1.2,
        ),
      ),
    ),
    onSelected: () => execute('window.fullscreen'),
  ),
];

MenuCustom _row(List<String> commands, ValueChanged<String> execute) =>
    MenuCustom(
      height: windowMenuRow,
      width: windowMenuWidth,
      builder: (context, close) => Stack(
        children: [
          for (var i = 0; i < commands.length; i++)
            Positioned(
              left: windowMenuMargin + windowMenuPitch * i,
              width: windowMenuPitch,
              top: 0,
              bottom: 0,
              child: _TileButton(
                key: ValueKey(commands[i]),
                glyph: tileGlyphs[commands[i]] ?? const [],
                onPressed: () {
                  close();
                  execute(commands[i]);
                },
              ),
            ),
        ],
      ),
    );

/// The icon glyph: light `#1d1d1f`, dark `#e1e1e1` (lightest pixel of
/// the dark capture, windows.md). The background under the pointer in dark is *estimated*.
const Color tileGlyph = Color(0xFF1D1D1F);
const Color tileGlyphDark = Color(0xFFE1E1E1);

/// An icon of the row: dark, with a light background under the pointer.
class _TileButton extends StatefulWidget {
  const _TileButton({super.key, required this.glyph, required this.onPressed});
  final TileGlyph glyph;
  final VoidCallback onPressed;

  @override
  State<_TileButton> createState() => _TileButtonState();
}

class _TileButtonState extends State<_TileButton> {
  bool _hover = false;

  @override
  Widget build(BuildContext context) {
    final dark =
        MediaQuery.maybePlatformBrightnessOf(context) == Brightness.dark;
    return MouseRegion(
      onEnter: (_) => setState(() => _hover = true),
      onExit: (_) => setState(() => _hover = false),
      child: GestureDetector(
        behavior: HitTestBehavior.opaque,
        onTap: widget.onPressed,
        child: Align(
          alignment: Alignment.center,
          child: Container(
            width: windowMenuHover.width,
            height: windowMenuHover.height,
            decoration: BoxDecoration(
              color: _hover
                  ? (dark ? const Color(0x1FFFFFFF) : const Color(0x14000000))
                  : null,
              borderRadius: BorderRadius.circular(7),
            ),
            alignment: Alignment.center,
            child: SizedBox.fromSize(
              size: windowMenuIcon,
              child: CustomPaint(
                painter: TilePainter(
                  widget.glyph,
                  dark ? tileGlyphDark : tileGlyph,
                ),
              ),
            ),
          ),
        ),
      ),
    );
  }
}

/// The screen outline (radius 4, stroke 1.5) with the filled parts: the chosen window in
/// the text color, the others in grey.
class TilePainter extends CustomPainter {
  const TilePainter(this.glyph, this.color, {this.stroke = 1.5});
  final TileGlyph glyph;
  final Color color;
  final double stroke;

  @override
  void paint(Canvas canvas, Size size) {
    final outer = RRect.fromRectAndRadius(
      (Offset.zero & size).deflate(stroke / 2),
      Radius.circular(size.height * 0.2),
    );
    canvas.drawRRect(
      outer,
      Paint()
        ..color = color
        ..style = PaintingStyle.stroke
        ..strokeWidth = stroke,
    );
    final inner = (Offset.zero & size).deflate(stroke + 1);
    canvas.save();
    canvas.clipRRect(
      RRect.fromRectAndRadius(inner, Radius.circular(size.height * 0.12)),
    );
    for (var i = 0; i < glyph.length; i++) {
      final part = glyph[i];
      final rect = Rect.fromLTWH(
        inner.left + part.left * inner.width,
        inner.top + part.top * inner.height,
        part.width * inner.width,
        part.height * inner.height,
      ).deflate(glyph.length > 1 ? 0.75 : 0);
      canvas.drawRect(
        rect,
        Paint()..color = i == 0 ? color : color.withValues(alpha: 0.35),
      );
    }
    canvas.restore();
  }

  @override
  bool shouldRepaint(TilePainter old) =>
      old.glyph != glyph || old.color != color || old.stroke != stroke;
}
