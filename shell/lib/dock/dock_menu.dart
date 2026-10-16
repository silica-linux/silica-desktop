// The Dock's menus on Amalfi: the rows are silica_ui's `AmalfiMenuList` (the rows of
// `AmalfiMenu`: Callout items, 24 pt, highlight `Accent/Amalfi` radius 7, submenus), on a
// Thick glass surface that Ossido draws (`menu` kind, radius 12, 4 pt padding) instead of
// the Flutter-side glass (a shell surface has no backdrop of its own). The arrow's outline is the
// Dock's own (`DockArrowBorder`, 6 pt deep), which `AmalfiGlass` cannot express, so the body and
// its arrow are a `ProfiledGlass`; the rows and the submenus (`AmalfiMenu`, glass group + 1, under
// the compositor backend) draw no glass of their own.
//
// The menu sits above the item it belongs to (kit `Dock/Menu`): its arrow, 6 pt deep under the
// body, has its tip [menuAboveShelf] pt over the shelf (12 at the default size) and centred on
// the item (the body is clamped to the screen, the arrow keeps pointing at the item). It fades
// in and out with `fade.instant`.

import 'dart:math' as math;

import 'package:flutter/gestures.dart';
import 'package:flutter/widgets.dart';
import 'package:silica_ui/silica_ui.dart';

import '../system/glass_profile.dart';
import 'arrow.dart';

/// The key of the menu's surface (glass, shadow and rows).
const Key dockMenuSurface = ValueKey('dock menu');

/// The tip of the menu's arrow above the shelf's top, at the default size (`DockMetrics.labelOffset`
/// scales it).
const double menuAboveShelf = 12;

/// Distance kept from the screen's sides.
const double menuScreenMargin = 8;

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
    this.onSubmenusChanged,
  });

  final List<MenuEntry> entries;

  /// The centre of the item the menu belongs to, in the host's coordinates.
  final double anchorX;

  /// The y of the tip of the menu's arrow (the bottom edge of the menu's frame).
  final double bottom;

  /// The glass group of the surface.
  final int group;

  /// The menu is gone (a choice, Esc or a click outside), once the fade has finished.
  final VoidCallback? onDismissed;

  /// The menu's rectangle (with its shadow's room excluded) changed.
  final ValueChanged<Rect>? onRectChanged;

  /// The rectangles of the open submenus (nested ones included), in global coordinates, when
  /// they change: the Dock's input region follows them.
  final ValueChanged<List<Rect>>? onSubmenusChanged;

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

  /// The x of the arrow's tip from the left of the menu; null until the menu is placed (the
  /// middle).
  double? _tipX;

  /// The rectangle of the surface, in the host's coordinates.
  Rect? get rect => _layout.rect;

  /// The open submenus' rectangles, as the list reports them.
  List<Rect> _submenus = const [];

  final FocusNode _focus = FocusNode(
    debugLabel: 'dock menu',
    canRequestFocus: false,
    skipTraversal: true,
  );

  /// Whether the keyboard focus is on the menu's rows (or on a submenu): they close the menu
  /// on Esc themselves, and close a submenu before the menu.
  bool get hasFocus => _focus.hasFocus;

  /// Whether `p` (host coordinates) is on the menu or on an open submenu.
  bool contains(Offset p) {
    final r = _layout.rect;
    if (r == null) return false;
    return r.inflate(2).contains(p) ||
        _submenus.any((s) => s.inflate(2).contains(p));
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
    _focus.dispose();
    super.dispose();
  }

  void _report() {
    final r = _layout.rect;
    if (r == null || r == _reported) return;
    _reported = r;
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (!mounted) return;
      final tip = widget.anchorX - r.left;
      if (_tipX == null || (_tipX! - tip).abs() > 0.01) {
        setState(() => _tipX = tip);
      }
      widget.onRectChanged?.call(r);
    });
  }

  @override
  Widget build(BuildContext context) => Focus(
    focusNode: _focus,
    child: Overlay(initialEntries: [_entry]),
  );

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
              child: Padding(
                padding: const EdgeInsets.only(bottom: dockArrowDepth),
                child: CustomPaint(
                  key: dockMenuSurface,
                  painter: _MenuShadow(shadows, _tipX),
                  child: ProfiledGlass(
                    profile: ShellGlassProfile.menu,
                    radius: AmalfiMenuMetrics.radius,
                    material: GlassMaterial.thick,
                    group: widget.group,
                    tip: _tipX == null
                        ? null
                        : GlassTip(
                            x: _tipX!,
                            width: dockArrowHalfBase * 2,
                            height: dockArrowDepth,
                          ),
                    child: Padding(
                      padding: const EdgeInsets.all(AmalfiMenuMetrics.padding),
                      child: AmalfiMenuList(
                        entries: _rows,
                        autofocus: true,
                        group: widget.group,
                        onDismiss: close,
                        onSubmenusChanged: (rects) {
                          _submenus = rects;
                          widget.onSubmenusChanged?.call(rects);
                        },
                      ),
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
  const _MenuShadow(this.shadows, this.tipX);
  final List<BoxShadow> shadows;
  final double? tipX;

  @override
  void paint(Canvas canvas, Size size) {
    if (tipX == null) {
      return paintAmalfiShadows(
        canvas,
        Offset.zero & size,
        shadows,
        shape: const AmalfiSmoothBorder(radius: AmalfiMenuMetrics.radius),
      );
    }
    // The shape with its arrow (the frame is the body and the arrow under it).
    final frame = Rect.fromLTWH(0, 0, size.width, size.height + dockArrowDepth);
    paintAmalfiShadows(
      canvas,
      frame,
      shadows,
      shape: DockArrowBorder(radius: AmalfiMenuMetrics.radius, tipX: tipX),
    );
  }

  @override
  bool shouldRepaint(_MenuShadow old) =>
      old.shadows != shadows || old.tipX != tipX;
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
