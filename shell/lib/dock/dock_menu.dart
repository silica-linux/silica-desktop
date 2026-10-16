// The Dock's menus on Amalfi: the rows are silica_ui's `AmalfiMenuList` (the rows of
// `AmalfiMenu`: Callout items, 24 pt, highlight `Accent/Amalfi` radius 7, submenus), on a
// Thick glass surface that Ossido draws (`menu` kind, radius 12, 4 pt padding) instead of
// `AmalfiMenu`'s own Flutter-side glass (a shell surface has no backdrop of its own).
//
// The menu sits above the item it belongs to, with its bottom [menuAboveShelf] pt over the
// shelf and centred on the item (clamped to the screen): Amalfi menus have no tip. It fades in
// and out with `fade.instant`.

import 'dart:math' as math;

import 'package:flutter/gestures.dart';
import 'package:flutter/widgets.dart';
import 'package:silica_ui/silica_ui.dart';

import '../system/glass_profile.dart';

/// The key of the menu's surface (glass, shadow and rows).
const Key dockMenuSurface = ValueKey('dock menu');

/// The menu's bottom edge above the shelf's top.
const double menuAboveShelf = 8;

/// Distance kept from the screen's sides.
const double menuScreenMargin = 8;

/// How far a submenu can reach beyond its parent, for the input region (the submenu is
/// placed by `AmalfiMenuList` in an overlay of its own, whose size the Dock does not see).
const double menuSubmenuReach = 320;

/// The Dock's `MenuEntry`s as Amalfi rows.
List<AmalfiMenuEntry> amalfiMenuEntries(List<MenuEntry> entries) => [
  for (final entry in entries)
    switch (entry) {
      MenuAction() => AmalfiMenuAction(
        entry.label,
        shortcut: entry.shortcut,
        icon: switch (entry.icon) {
          final icon? => Builder(
            builder: (context) => icon(
              IconTheme.of(context).color ??
                  AmalfiTheme.of(context).colors.textPrimary,
            ),
          ),
          null => null,
        },
        checked: entry.checked,
        enabled: entry.enabled,
        onSelected: entry.onSelected,
        submenu: entry.submenu == null
            ? null
            : amalfiMenuEntries(entry.submenu!),
      ),
      MenuSection() => AmalfiMenuSection(entry.label),
      MenuDivider() || MenuCustom() => const AmalfiMenuDivider(),
    },
];

/// Where a submenu of the menu in `menu` can be (its overlay is not measured): beside it, from
/// the top of the surface down to `bottom` and beyond.
Rect submenuBand(Rect menu, {required double bottom}) => Rect.fromLTRB(
  math.max(0, menu.left - menuSubmenuReach),
  0,
  menu.right + menuSubmenuReach,
  bottom + 1000,
);

/// Whether any row opens a submenu.
bool hasSubmenu(List<MenuEntry> entries) =>
    entries.any((e) => e is MenuAction && e.submenu != null);

/// The open Dock menu: glass surface and rows, placed above [anchorX].
class DockMenuHost extends StatefulWidget {
  const DockMenuHost({
    super.key,
    required this.entries,
    required this.anchorX,
    required this.bottom,
    required this.group,
    this.onDismissed,
    this.onRectChanged,
  });

  final List<MenuEntry> entries;

  /// The centre of the item the menu belongs to, in the host's coordinates.
  final double anchorX;

  /// The y of the menu's bottom edge.
  final double bottom;

  /// The glass group of the surface.
  final int group;

  /// The menu is gone (a choice, Esc or a click outside), once the fade has finished.
  final VoidCallback? onDismissed;

  /// The menu's rectangle (with its shadow's room excluded) changed.
  final ValueChanged<Rect>? onRectChanged;

  @override
  State<DockMenuHost> createState() => DockMenuHostState();
}

class DockMenuHostState extends State<DockMenuHost>
    with SingleTickerProviderStateMixin {
  late final AnimationController _fade = AnimationController(
    vsync: this,
    duration: AmalfiFades.instant.duration,
  )..forward();
  final _DockMenuLayout _layout = _DockMenuLayout();
  Rect? _reported;
  bool _closing = false;
  int _synthetic = 0;

  /// The rectangle of the surface, in the host's coordinates.
  Rect? get rect => _layout.rect;

  /// Whether `p` (host coordinates) is on the menu or, while a submenu may be open, in the
  /// band where it would be.
  bool contains(Offset p) {
    final r = _layout.rect;
    if (r == null) return false;
    if (r.inflate(2).contains(p)) return true;
    return hasSubmenu(widget.entries) &&
        submenuBand(r, bottom: widget.bottom).contains(p);
  }

  /// Fades out and calls `onDismissed`.
  void close() {
    if (_closing) return;
    _closing = true;
    final done = widget.onDismissed;
    _fade.animateBack(0, curve: AmalfiFades.instant.curve).whenComplete(() {
      if (mounted) done?.call();
    });
  }

  // The menu takes the real pointer itself (hover and clicks reach its rows). Only a press
  // that began on the Dock's icon and is released over a row (the long press, D-04) has to be
  // handed in: it is replayed as a click on the row under the pointer.
  static const int _device = 7931;

  void pointerMoved(Offset p) {
    if (_closing) return;
    _send(
      PointerHoverEvent(
        device: _device,
        position: p,
        kind: PointerDeviceKind.mouse,
      ),
    );
  }

  void pointerUp(Offset p) {
    if (_closing) return;
    _send(
      PointerHoverEvent(
        device: _device,
        position: p,
        kind: PointerDeviceKind.mouse,
      ),
    );
    final pointer = 9000 + _synthetic++;
    _send(
      PointerDownEvent(
        pointer: pointer,
        device: _device,
        position: p,
        kind: PointerDeviceKind.mouse,
        buttons: kPrimaryButton,
      ),
    );
    _send(
      PointerUpEvent(
        pointer: pointer,
        device: _device,
        position: p,
        kind: PointerDeviceKind.mouse,
      ),
    );
  }

  void _send(PointerEvent event) {
    final view = View.maybeOf(context);
    if (view == null) return;
    final binding = GestureBinding.instance;
    if (event is PointerHoverEvent && _synthetic == 0 && !_added) {
      _added = true;
      binding.handlePointerEvent(
        PointerAddedEvent(
          device: _device,
          position: event.position,
          kind: PointerDeviceKind.mouse,
          viewId: view.viewId,
        ),
      );
    }
    binding.handlePointerEvent(_withView(event, view.viewId));
  }

  bool _added = false;

  PointerEvent _withView(PointerEvent e, int viewId) => switch (e) {
    PointerHoverEvent() => PointerHoverEvent(
      device: e.device,
      position: e.position,
      kind: e.kind,
      viewId: viewId,
    ),
    PointerDownEvent() => PointerDownEvent(
      pointer: e.pointer,
      device: e.device,
      position: e.position,
      kind: e.kind,
      buttons: e.buttons,
      viewId: viewId,
    ),
    PointerUpEvent() => PointerUpEvent(
      pointer: e.pointer,
      device: e.device,
      position: e.position,
      kind: e.kind,
      viewId: viewId,
    ),
    _ => e,
  };

  // The rows open their submenus in an overlay (`AmalfiMenuList`): the Dock surface has none,
  // so the host brings its own, as big as the surface.
  late final OverlayEntry _entry = OverlayEntry(builder: _buildMenu);

  // The rows are converted once per list of entries: the list resets its highlight and its
  // submenu when it is handed new rows, and the Dock rebuilds the host at every pointer move.
  late List<AmalfiMenuEntry> _rows = amalfiMenuEntries(widget.entries);

  @override
  void didUpdateWidget(DockMenuHost old) {
    super.didUpdateWidget(old);
    if (!identical(old.entries, widget.entries)) {
      _rows = amalfiMenuEntries(widget.entries);
    }
    _entry.markNeedsBuild();
  }

  @override
  void dispose() {
    _entry
      ..remove()
      ..dispose();
    _fade.dispose();
    super.dispose();
  }

  void _report() {
    final r = _layout.rect;
    if (r == null || r == _reported) return;
    _reported = r;
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (mounted) widget.onRectChanged?.call(r);
    });
  }

  @override
  Widget build(BuildContext context) => Overlay(initialEntries: [_entry]);

  Widget _buildMenu(BuildContext context) {
    final theme = AmalfiTheme.of(context);
    final shadows = AmalfiGlass.shadowsOf(
      AmalfiMaterials.thick,
      theme.brightness,
    );
    _layout
      ..anchorX = widget.anchorX
      ..bottom = widget.bottom;
    WidgetsBinding.instance.addPostFrameCallback((_) => _report());
    return FadeTransition(
      opacity: _fade,
      child: CustomSingleChildLayout(
        delegate: _layout,
        child: Semantics(
          container: true,
          explicitChildNodes: true,
          child: ConstrainedBox(
            constraints: const BoxConstraints(
              minWidth: AmalfiMenuMetrics.minWidth,
            ),
            child: IntrinsicWidth(
              stepWidth: AmalfiGrid.base,
              child: CustomPaint(
                key: dockMenuSurface,
                painter: _MenuShadow(shadows),
                child: ProfiledGlass(
                  profile: ShellGlassProfile.menu,
                  radius: AmalfiMenuMetrics.radius,
                  material: GlassMaterial.thick,
                  group: widget.group,
                  child: Padding(
                    padding: const EdgeInsets.all(AmalfiMenuMetrics.padding),
                    child: AmalfiMenuList(
                      entries: _rows,
                      autofocus: true,
                      onDismiss: close,
                    ),
                  ),
                ),
              ),
            ),
          ),
        ),
      ),
    );
  }
}

class _MenuShadow extends CustomPainter {
  const _MenuShadow(this.shadows);
  final List<BoxShadow> shadows;

  @override
  void paint(Canvas canvas, Size size) => paintAmalfiShadows(
    canvas,
    Offset.zero & size,
    shadows,
    shape: const AmalfiSmoothBorder(radius: AmalfiMenuMetrics.radius),
  );

  @override
  bool shouldRepaint(_MenuShadow old) => old.shadows != shadows;
}

/// Places the menu above its item, centred and kept on screen; remembers where.
class _DockMenuLayout extends SingleChildLayoutDelegate {
  double anchorX = 0;
  double bottom = 0;
  Rect? rect;

  @override
  BoxConstraints getConstraintsForChild(BoxConstraints constraints) =>
      BoxConstraints(
        maxWidth: math.max(0, constraints.maxWidth - 2 * menuScreenMargin),
        maxHeight: math.max(0, bottom - 2 * menuScreenMargin),
      );

  @override
  Offset getPositionForChild(Size size, Size childSize) {
    final x = (anchorX - childSize.width / 2)
        .clamp(
          menuScreenMargin,
          math.max(
            menuScreenMargin,
            size.width - menuScreenMargin - childSize.width,
          ),
        )
        .toDouble();
    final y = math.max(menuScreenMargin, bottom - childSize.height);
    rect = Rect.fromLTWH(x, y, childSize.width, childSize.height);
    return Offset(x, y);
  }

  @override
  bool shouldRelayout(_DockMenuLayout old) => true;
}
