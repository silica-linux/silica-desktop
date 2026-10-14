// Edit Controls (docs/spec/controlcenter.md › Edit Controls, CC-04, SH-CC-21…23): the
// gallery of controls beside the screen's centre, and the pieces the Center gains in edit
// mode (remove badges, empty round slots, the dimmed screen).
//
// Gallery: 736 × 900 pt, centred, light glass with ~14 pt corners. Left column on `#c8c9ce`:
// the 205 × 28 search field (`#d7d8db`) and 205 × 46 category rows (coloured icon square and
// name, the chosen one on a `#b4b5b9` capsule). Right part (494 pt) on `#a6a8ae`: one section
// per category with a 15 pt header and each control previewed at its size with its name
// below; a two-line hint and the blue 54 × 28 Done button at the bottom.
//
// Adding: dragging a preview into the Center (onto the stack or an empty slot), as in the
// reference, or clicking it (provisional: the reference's click behaviour was not observed).
// Controls already in the Center are dimmed in the gallery.

import 'package:flutter/widgets.dart';
import 'package:silica_ui/silica_ui.dart';

import '../l10n/l10n.dart';
import 'glyphs.dart';
import 'modules.dart';

const Size gallerySize = Size(736, 900);
const double galleryRadius = 14;
const double galleryColumn = 205;
const double galleryPad = 20;
const double galleryRow = 46;
const Color galleryLeftFill = Color(0xFFC8C9CE);
const Color galleryRightFill = Color(0xFFA6A8AE);
const Color gallerySearchFill = Color(0xFFD7D8DB);
const Color gallerySelected = Color(0xFFB4B5B9);
const Color galleryText = Color(0xFF1D1D1F);
const Color galleryTextDim = Color(0xFF55565B);

/// The Done button: 54 × 28, the reference's blue (raw `#296ffb`).
const Size doneSize = Size(54, 28);
const Color doneFill = Color(0xFF2A6FFB);

/// Remove badge: 20 × 20, 2 pt outside the module's top-left corner; `#3c3c3d` disc with a
/// `#c6c5ca` minus.
const double badgeSide = 20;
const double badgeOutside = 2;
const Color badgeFill = Color(0xFF3C3C3D);
const Color badgeGlyph = Color(0xFFC6C5CA);

/// Empty slots: eight 64 pt discs `#8c8d91` in two rows of four under the stack.
const int emptySlots = 8;
const Color emptySlotFill = Color(0xFF8C8D91);

/// The screen behind is dimmed by ~12 levels (`#7a7b80` → `#6e6f73`).
const Color editDim = Color(0x1A000000);

/// Timing (from the click): gallery from +185 ms, settled at +345; the empty slots from
/// +412 to +530; closing, the gallery fades in ~240 ms.
const double editTotalMs = 530;
double galleryOpacity(double ms) => ((ms - 185) / 160).clamp(0.0, 1.0);
double slotsOpacity(double ms) => ((ms - 412) / 118).clamp(0.0, 1.0);

/// The gallery's categories and their controls, in order.
enum GalleryCategory {
  connectivity(['wifi', 'bluetooth', 'nearby'], Color(0xFF0A84FF)),
  focus(['dnd'], Color(0xFF6E7CFF)),
  nowPlaying(['media'], Color(0xFFFF375F)),
  display(['display', 'keyboard', 'dark', 'mirroring'], Color(0xFF0A84FF)),
  sound(['sound'], Color(0xFFFF453A)),
  screenshot(['capture'], Color(0xFF8E8E93)),
  desktop(['staging'], Color(0xFF30B0C7)),
  battery(['battery'], Color(0xFF30D158));

  const GalleryCategory(this.controls, this.color);
  final List<String> controls;
  final Color color;
}

/// A remove badge on a module's corner in edit mode.
class RemoveBadge extends StatelessWidget {
  const RemoveBadge({super.key, required this.label, required this.onTap});

  final String label;
  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) => Semantics(
    button: true,
    label: label,
    child: GestureDetector(
      onTap: onTap,
      behavior: HitTestBehavior.opaque,
      child: const DecoratedBox(
        decoration: BoxDecoration(shape: BoxShape.circle, color: badgeFill),
        child: Center(
          child: Glyph(GlyphKind.minus, color: badgeGlyph, size: 14),
        ),
      ),
    ),
  );
}

/// One empty round slot: a drop target for a control from the gallery.
class EmptySlot extends StatelessWidget {
  const EmptySlot({super.key, required this.onDrop});

  final ValueChanged<String> onDrop;

  @override
  Widget build(BuildContext context) => DragTarget<String>(
    onAcceptWithDetails: (d) => onDrop(d.data),
    builder: (context, candidates, _) => DecoratedBox(
      decoration: BoxDecoration(
        shape: BoxShape.circle,
        color: candidates.isEmpty
            ? emptySlotFill
            : emptySlotFill.withValues(alpha: 0.6),
      ),
    ),
  );
}

TextStyle _galleryText(
  double size, {
  FontWeight? weight,
  Color color = galleryText,
}) => TextStyle(
  fontFamily: uiFontFamily,
  package: 'silica_ui',
  fontSize: size,
  fontWeight: weight,
  color: color,
);

/// The gallery of controls.
class ControlGallery extends StatefulWidget {
  const ControlGallery({
    super.key,
    required this.present,
    required this.available,
    required this.preview,
    required this.label,
    required this.onAdd,
    required this.onDone,
  });

  /// The ids already in the Center.
  final Set<String> present;

  /// The ids this machine can show (the battery only with a battery).
  final Set<String> available;

  /// A control drawn at its first size, without interaction.
  final Widget Function(String id, ModuleSize size) preview;

  /// The control's name.
  final String Function(String id) label;
  final ValueChanged<String> onAdd;
  final VoidCallback onDone;

  @override
  State<ControlGallery> createState() => _ControlGalleryState();
}

class _ControlGalleryState extends State<ControlGallery> {
  /// Null: all controls.
  GalleryCategory? _category;
  final TextEditingController _search = TextEditingController();
  final FocusNode _searchFocus = FocusNode();

  @override
  void dispose() {
    _search.dispose();
    _searchFocus.dispose();
    super.dispose();
  }

  String _categoryName(AppLocalizations l, GalleryCategory? c) => switch (c) {
    null => l.ccAllControls,
    GalleryCategory.connectivity => l.ccCategoryConnectivity,
    GalleryCategory.focus => l.ccFocus,
    GalleryCategory.nowPlaying => l.ccNowPlaying,
    GalleryCategory.display => l.ccCategoryDisplay,
    GalleryCategory.sound => l.ccSound,
    GalleryCategory.screenshot => l.ccScreenCapture,
    GalleryCategory.desktop => l.ccCategoryDesktop,
    GalleryCategory.battery => l.ccBattery,
  };

  List<GalleryCategory> get _categories => [
    for (final c in GalleryCategory.values)
      if (c.controls.any(widget.available.contains)) c,
  ];

  @override
  Widget build(BuildContext context) {
    final l = l10nOf(context);
    return ClipRRect(
      borderRadius: BorderRadius.circular(galleryRadius),
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          SizedBox(
            width: galleryColumn + 2 * galleryPad,
            child: ColoredBox(color: galleryLeftFill, child: _sidebar(l)),
          ),
          Expanded(
            child: ColoredBox(color: galleryRightFill, child: _content(l)),
          ),
        ],
      ),
    );
  }

  Widget _sidebar(AppLocalizations l) => Padding(
    padding: const EdgeInsets.all(galleryPad),
    child: Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        SizedBox(
          height: 28,
          child: DecoratedBox(
            decoration: BoxDecoration(
              color: gallerySearchFill,
              borderRadius: BorderRadius.circular(8),
            ),
            child: Padding(
              padding: const EdgeInsets.symmetric(horizontal: 8),
              child: Stack(
                alignment: Alignment.centerLeft,
                children: [
                  if (_search.text.isEmpty)
                    IgnorePointer(
                      child: Text(
                        l.ccSearchControls,
                        style: _galleryText(13, color: galleryTextDim),
                      ),
                    ),
                  EditableText(
                    controller: _search,
                    focusNode: _searchFocus,
                    style: _galleryText(13),
                    cursorColor: doneFill,
                    backgroundCursorColor: galleryTextDim,
                    onChanged: (_) => setState(() {}),
                  ),
                ],
              ),
            ),
          ),
        ),
        const SizedBox(height: 8),
        for (final c in [null, ..._categories]) _categoryRow(l, c),
      ],
    ),
  );

  Widget _categoryRow(AppLocalizations l, GalleryCategory? c) {
    final selected = c == _category;
    return GestureDetector(
      onTap: () => setState(() => _category = c),
      behavior: HitTestBehavior.opaque,
      child: SizedBox(
        height: galleryRow,
        child: DecoratedBox(
          decoration: BoxDecoration(
            color: selected ? gallerySelected : null,
            borderRadius: BorderRadius.circular(galleryRow / 2),
          ),
          child: Row(
            children: [
              const SizedBox(width: 10),
              Container(
                width: 26,
                height: 26,
                decoration: BoxDecoration(
                  color: c?.color ?? const Color(0xFF8E8E93),
                  borderRadius: BorderRadius.circular(6),
                ),
              ),
              const SizedBox(width: 10),
              Expanded(
                child: Text(
                  _categoryName(l, c),
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                  style: _galleryText(13, weight: FontWeight.w500),
                ),
              ),
            ],
          ),
        ),
      ),
    );
  }

  Widget _content(AppLocalizations l) {
    final query = _search.text.trim().toLowerCase();
    final sections = [
      for (final c in _category == null ? _categories : [_category!])
        (
          c,
          [
            for (final id in c.controls)
              if (widget.available.contains(id) &&
                  (query.isEmpty ||
                      widget.label(id).toLowerCase().contains(query)))
                id,
          ],
        ),
    ].where((s) => s.$2.isNotEmpty);
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Expanded(
          child: ListView(
            padding: const EdgeInsets.all(galleryPad),
            children: [
              for (final (category, ids) in sections) ...[
                Padding(
                  padding: const EdgeInsets.only(bottom: 10, top: 6),
                  child: Text(
                    _categoryName(l, category),
                    style: _galleryText(15, weight: FontWeight.w700),
                  ),
                ),
                Wrap(
                  spacing: 16,
                  runSpacing: 14,
                  children: [for (final id in ids) _item(l, id)],
                ),
                const SizedBox(height: 18),
              ],
            ],
          ),
        ),
        Padding(
          padding: const EdgeInsets.fromLTRB(galleryPad, 8, galleryPad, 16),
          child: Row(
            children: [
              Expanded(
                child: Text(
                  l.ccGalleryHint,
                  maxLines: 2,
                  style: _galleryText(12, color: galleryTextDim),
                ),
              ),
              const SizedBox(width: 16),
              GestureDetector(
                onTap: widget.onDone,
                child: SizedBox.fromSize(
                  size: doneSize,
                  child: DecoratedBox(
                    decoration: BoxDecoration(
                      color: doneFill,
                      borderRadius: BorderRadius.circular(doneSize.height / 2),
                    ),
                    child: Center(
                      child: Text(
                        l.ccDone,
                        style: _galleryText(
                          13,
                          weight: FontWeight.w600,
                          color: const Color(0xFFFFFFFF),
                        ),
                      ),
                    ),
                  ),
                ),
              ),
            ],
          ),
        ),
      ],
    );
  }

  Widget _item(AppLocalizations l, String id) {
    final size = moduleSizes[id]?.first ?? ModuleSize.small;
    final extent = size == ModuleSize.wide ? const Size(292, 64) : size.extent;
    final present = widget.present.contains(id);
    final shape = _previewShape(id, size, extent);
    final item = Column(
      mainAxisSize: MainAxisSize.min,
      children: [
        shape,
        const SizedBox(height: 4),
        SizedBox(
          width: extent.width,
          child: Text(
            widget.label(id),
            semanticsLabel: present
                ? '${widget.label(id)}, ${l.ccInCenter}'
                : widget.label(id),
            maxLines: 1,
            overflow: TextOverflow.ellipsis,
            textAlign: TextAlign.center,
            style: _galleryText(11, weight: FontWeight.w500),
          ),
        ),
      ],
    );
    if (present) return Opacity(opacity: 0.5, child: item);
    // The drag starts at once (it wins the gesture arena), so a click is told apart by
    // the pointer: released within a few points of where it went down.
    Offset? down;
    return Listener(
      onPointerDown: (e) => down = e.position,
      onPointerUp: (e) {
        final start = down;
        down = null;
        if (start != null && (e.position - start).distance < clickSlop) {
          widget.onAdd(id);
        }
      },
      child: Draggable<String>(
        data: id,
        feedback: Opacity(opacity: 0.85, child: shape),
        childWhenDragging: Opacity(opacity: 0.4, child: item),
        child: Semantics(
          button: true,
          label: widget.label(id),
          onTap: () => widget.onAdd(id),
          child: item,
        ),
      ),
    );
  }

  /// How far the pointer may move for a press to still count as a click.
  static const double clickSlop = 4;

  /// The control on the module fill, at its size.
  Widget _previewShape(String id, ModuleSize size, Size extent) =>
      SizedBox.fromSize(
        size: extent,
        child: DecoratedBox(
          decoration: BoxDecoration(
            color: const Color(0xFF6A6B70),
            borderRadius: BorderRadius.circular(switch (size) {
              ModuleSize.small || ModuleSize.medium => 32,
              ModuleSize.large => 35,
              ModuleSize.wide => 28,
            }),
          ),
          child: IgnorePointer(child: widget.preview(id, size)),
        ),
      );
}
