// Banners and Notification Center, from plan/reference/notifications.md (light appearance).
//
// A 344 pt wide column 16 pt from the right edge and 16 pt below the menu bar. "Notification"
// glass cards (σ ~10.5 pt, almost opaque) with 20 pt corners: 32 pt icon 14 pt from the
// edge, text from 58 pt, padding 12 above and 13 below, semibold title and regular body at
// ~13.75 pt (10 pt capitals), same color. The banner enters from the right with the measured
// curve; the Center (notifications.toggle command, click on the clock) shows the
// notifications by app, with stacked cards, the actions and a wide scrim around the
// column.
//
// The component keeps running on a full-screen overlay layer: without banners or the
// Center it takes neither pointer nor keyboard (silica/layer).

import 'dart:async';
import 'dart:io';

import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_svg/flutter_svg.dart';
import 'package:silica_ui/silica_ui.dart';

import '../menubar/menubar.dart' show barHeight;
import '../l10n/l10n.dart';
import '../system/cross_glyph.dart';
import '../system/desktop_entries.dart';
import '../system/icon_lookup.dart';
import '../system/layer_control.dart';
import '../system/shadow_image.dart';
import '../system/shell_channel.dart';
import '../system/system_status.dart';
import 'banner.dart';
import 'model.dart';
import 'service.dart';
import 'settings.dart';

const double columnWidth = 344;
const double columnMargin = 16;

/// The surface covers the whole screen (exclusive zone -1): the column starts below the
/// menu bar.
const double columnTop = barHeight + columnMargin;
const double cardRadius = 20;
const double iconSize = 32;
const double iconLeft = 14;
const double textLeft = 58;
const double paddingRight = 14;

/// Measured padding: 12 above the title line, 13 below the last one. At equal line height
/// Inter puts the capitals 2 pt higher: with 14 the title capitals are 15 pt from the edge
/// as in the reference (y 61 over 46).
const double paddingTop = 14;
const double paddingBottom = 13;

/// Text size: capitals measure 10 pt, Inter has them at 0.727 em.
const double fontSize = 13.75;
const double lineHeight = 16.5;
const double smallSize = 11;

/// Title, subtitle and body share the same color (`#1d1d1e` in the darkest pixels).
const Color textColor = Color(0xFF1D1D1E);

/// Time and secondary texts (`#575759`).
const Color secondaryColor = Color(0xFF575759);

/// Center header: almost solid white over the scrim (`#e1e1e1` over `#5e5f63`).
const Color headerColor = Color(0xE0FFFFFF);

/// Center header: 21 pt, 15.5 pt below the card above, 7.5 pt above the first.
const double headerHeight = 21;
const double headerSize = 18.5;
const double headerGapBelow = 7.5;

/// Between the most recent card (above the header) and the header.
const double headerGapAbove = 15.5;

/// From the last card to the footer (pill and clear menu), and between the two.
const double footerGap = 16;
const double footerSpacing = 8;
const Size widgetsPillSize = Size(91, 24);
const double clearMenuSize = 24;
const double widgetsPillTextSize = 11;

/// Empty Center: the text 30 pt below the column top, 17 pt tall, the pill 33 pt under it.
const double emptyTextTop = 30;
const double emptyTextHeight = 17;
const double emptyPillGap = 33;

/// A footer control without a backend yet.
const double disabledFooterOpacity = 0.45;

/// Space between cards in the Center (not measured: like the pitch of stacked cards).
const double cardGap = 8;

/// Expanded group (session 8, N-03): cards at a 64 pitch (6 pt between them), header with
/// the title at 19 pt and 20 pt controls; the shape swap between the two controls and the
/// expansion take ~150–200 ms.
const double expandedGap = 6;
const double groupHeaderHeight = 28;
const double groupTitleSize = 19;
const double groupPillHeight = 20;
const double groupCollapseWidth = 102;
const double groupClearWidth = 65;
const Duration groupMorph = Duration(milliseconds: 180);

/// Stacked cards under a collapsed group: 7 pt strips at an 8 pitch, inset by ~19 and
/// ~29 pt per side.
const double stackStep = 8;
const double stackStrip = 7;
const List<double> stackInsets = [19, 29];

/// Actions section: ~20 levels darker, separated by a 1 pt line; 22 pt capsule buttons,
/// two per row with 8 pt between them, 12 pt from the edges.
const Color actionsFill = Color(0x1E000000);
const Color actionsDivider = Color(0x38000000);
const Color buttonFill = Color(0x1C000000);
const Color buttonText = Color(0xFF2F2F31);
const double buttonHeight = 22;

/// The card colors, light and dark. Dark is measured in
/// silica-reference (glass.md, notifications: Center open over grey, lightest pixels of the
/// texts, backgrounds in clean spots) and on the dark banner over grey (text `#eaeaeb`):
/// - text `#eaeaeb`, time `#b4b5ba`, buttons `#dadbe0`;
/// - actions section **lighter** than the card by ~20 levels (`#7a7a7e` over `#66676c`,
///   the opposite of light), darker divider line (102 over 122), buttons +21 levels;
/// - × circle `#717277` with a `#36373a` rim and a `#bfc0c2` glyph; group capsules `#74757a`
///   with `#c2c3c8` text; stacked cards ~`#68686c` with the bottom line `#858589`.
/// The dark translucencies are derived from those levels over grey. The Center header is
/// the same in both appearances (`headerColor`).
@immutable
class NotificationColors {
  const NotificationColors({
    required this.text,
    required this.secondary,
    required this.actionsFill,
    required this.actionsDivider,
    required this.buttonFill,
    required this.buttonText,
    required this.closeFill,
    required this.closeBorder,
    required this.closeGlyph,
    required this.pillFill,
    required this.pillText,
    required this.stripFill,
    required this.stripLine,
  });

  final Color text;
  final Color secondary;
  final Color actionsFill;
  final Color actionsDivider;
  final Color buttonFill;
  final Color buttonText;
  final Color closeFill;
  final Color closeBorder;
  final Color closeGlyph;
  final Color pillFill;
  final Color pillText;
  final Color stripFill;
  final Color stripLine;

  static const NotificationColors light = NotificationColors(
    text: Color(0xFF1D1D1E),
    secondary: Color(0xFF575759),
    actionsFill: Color(0x1E000000),
    actionsDivider: Color(0x38000000),
    buttonFill: Color(0x1C000000),
    buttonText: Color(0xFF2F2F31),
    closeFill: Color(0xFFB4B5B9),
    closeBorder: Color(0xFF3A3B3E),
    closeGlyph: Color(0xFF5A5B5C),
    pillFill: Color(0xFFB4B5B9),
    pillText: Color(0xFF3A3B3E),
    stripFill: Color(0xE6ADAEB2),
    stripLine: Color(0xFFBDBEC2),
  );

  static const NotificationColors dark = NotificationColors(
    text: Color(0xFFEAEAEB),
    secondary: Color(0xFFB4B5BA),
    actionsFill: Color(0x21FFFFFF),
    actionsDivider: Color(0x2A000000),
    buttonFill: Color(0x28FFFFFF),
    buttonText: Color(0xFFDADBE0),
    closeFill: Color(0xFF717277),
    closeBorder: Color(0xFF36373A),
    closeGlyph: Color(0xFFBFC0C2),
    pillFill: Color(0xFF74757A),
    pillText: Color(0xFFC2C3C8),
    stripFill: Color(0xE668686C),
    stripLine: Color(0xFF858589),
  );

  static NotificationColors of(BuildContext context) =>
      MediaQuery.maybePlatformBrightnessOf(context) == Brightness.dark
      ? dark
      : light;
}

const double actionsInset = 12;

/// Buttons that appear when the pointer passes over the banner (session 8, N-02).
const double closeSize = 21;
const Offset closeCenter = Offset(4, 6);
const Size detailsSize = Size(65, 21);
const double detailsInset = 9;

/// Space between the shortened text and the capsule (not measured).
const double detailsGap = 8;

/// The Center scrim darkens the background by ~×0.75 between the elements and fades in ~60 pt.
const Color scrimColor = Color(0x40000000);
const double scrimBlur = 44;

/// Shadow of a lone banner: very faint (−4 levels below the edge, −2 at the sides),
/// downwards, ~15 pt. Only outside the card: inside it would cover the glass.
const Color shadowColor = Color(0x10000000);
const double shadowSigma = 6;
const Offset shadowOffset = Offset(0, 4);

TextStyle _style(double size, Color color, {FontWeight? weight}) => TextStyle(
  fontFamily: uiFontFamily,
  package: 'silica_ui',
  fontSize: size,
  fontWeight: weight,
  color: color,
  height: size == fontSize ? lineHeight / fontSize : null,
);

/// Opens the app that sent `n`: its desktop entry (`desktop-entry` hint), or an installed
/// app with the same name. Nothing when it cannot be found.
void openSenderApp(ShellNotification n) {
  final entries = loadDesktopEntries();
  final byId = n.desktopEntry.isEmpty
      ? null
      : entries[n.desktopEntry] ??
            entries[n.desktopEntry.replaceAll(RegExp(r'\.desktop$'), '')];
  final entry =
      byId ?? entries.values.where((e) => e.name == n.appName).firstOrNull;
  if (entry == null || entry.exec.isEmpty) return;
  ShellChannel.instance.launch(entry.exec);
}

/// `HH:MM` of the arrival time, as in the reference Center.
String timeLabel(DateTime t) =>
    '${t.hour.toString().padLeft(2, '0')}:${t.minute.toString().padLeft(2, '0')}';

class NotificationsLayer extends StatefulWidget {
  const NotificationsLayer({
    super.key,
    required this.source,
    this.system,
    this.startOpen = false,
    this.settings = NotificationSettings.read,
    this.openApp = openSenderApp,
  });

  /// A click on a notification without a `default` action opens its app (N-02).
  final void Function(ShellNotification) openApp;

  final NotificationsSource source;

  /// The per-app styles (banners, alerts, none), read again for every notification so a
  /// change in Settings applies at once.
  final NotificationSettings Function() settings;

  /// With Do Not Disturb on, notifications go to the Center without a banner.
  final SystemSource? system;
  final bool startOpen;

  @override
  State<NotificationsLayer> createState() => _NotificationsLayerState();
}

class _BannerEntry {
  _BannerEntry(this.notification, {this.alert = false});

  final Key key = UniqueKey();
  ShellNotification notification;

  /// Alerts style: it stays until the user acts on it.
  final bool alert;
  bool leaving = false;
}

class _NotificationsLayerState extends State<NotificationsLayer>
    with SingleTickerProviderStateMixin {
  /// Opening and closing of the Center: the value is the elapsed time, relative to the
  /// duration of the current phase.
  late final AnimationController _center;
  final FocusNode _focus = FocusNode();
  StreamSubscription<ShellNotification>? _posted;
  StreamSubscription<String>? _commands;

  /// The banners on screen: at most one entering or staying, the others leaving.
  final List<_BannerEntry> _banners = [];
  bool _centerOpen = false;

  /// The clear menu under the 24 pt circle of the footer.
  bool _clearMenu = false;
  final GlobalKey _clearMenuKey = GlobalKey();
  final Set<String> _expanded = {};

  /// The surface size at the last layout.
  Size? _size;

  @override
  void initState() {
    super.initState();
    _center = AnimationController(vsync: this)
      ..addListener(() => setState(() {}));
    widget.source.addListener(_changed);
    _posted = widget.source.posted.listen(_post);
    _commands = ShellChannel.instance.commands.listen((id) {
      if (id == 'notifications.toggle') _toggleCenter();
    });
    _dismissed = ShellChannel.instance.transientDismissed.listen((_) {
      if (!_centerOpen) return;
      _dismissedAt = DateTime.now();
      _closeCenter();
    });
    if (widget.startOpen) {
      _openCenter();
    } else {
      _updateInput();
    }
  }

  @override
  void dispose() {
    widget.source.removeListener(_changed);
    _posted?.cancel();
    _commands?.cancel();
    _dismissed?.cancel();
    _center.dispose();
    _focus.dispose();
    super.dispose();
  }

  void _changed() {
    // Closed elsewhere (from the Center, by the app, expired): the banner leaves.
    for (final banner in _banners) {
      if (!widget.source.notifications.containsKey(banner.notification.id)) {
        banner.leaving = true;
      }
    }
    setState(() {});
  }

  void _post(ShellNotification n) {
    // With a Focus (Do Not Disturb) on the notification is delivered silently, without a
    // banner, and waits in the Center (SH-NC-07, measured).
    if (_centerOpen || (widget.system?.value.doNotDisturb ?? false)) return;
    final style = widget.settings().styleOf(n);
    if (style == NotificationStyle.none) return;
    setState(() {
      final same = _banners.where(
        (b) => !b.leaving && b.notification.id == n.id,
      );
      if (same.isNotEmpty) {
        // Replaced by the app (same id): the content changes in place.
        same.first.notification = n;
        return;
      }
      // A new notification replaces the current banner, which leaves while it enters.
      for (final banner in _banners) {
        banner.leaving = true;
      }
      _banners.add(_BannerEntry(n, alert: style == NotificationStyle.alerts));
    });
    _updateInput();
  }

  void _gone(_BannerEntry banner) {
    setState(() => _banners.remove(banner));
    _updateInput();
  }

  // A press on the bar's clock while the Center is open: Ossido first ends the panel
  // (transientDismissed, the press goes through), then the bar asks for the toggle, which
  // must not open it again.
  void _toggleCenter() {
    if (!_centerOpen && _dismissedAt != null) {
      final since = DateTime.now().difference(_dismissedAt!);
      if (since < const Duration(milliseconds: 400)) return;
    }
    _centerOpen ? _closeCenter() : _openCenter();
  }

  StreamSubscription<void>? _dismissed;
  DateTime? _dismissedAt;

  void _openCenter() {
    setState(() {
      _banners.clear();
      _centerOpen = true;
      ShellChannel.instance.broadcast('notifications.open', '1');
    });
    _center
      ..duration = centerOpenDuration
      ..forward(from: 0);
    _updateInput();
    WidgetsBinding.instance.addPostFrameCallback((_) => _focus.requestFocus());
  }

  void _closeCenter() {
    if (!_centerOpen) return;
    setState(() {
      _centerOpen = false;
      ShellChannel.instance.broadcast('notifications.open', '0');
      _expanded.clear();
      _clearMenu = false;
    });
    _center
      ..duration = centerCloseDuration
      ..forward(from: 0);
    _updateInput();
  }

  /// The pointer is needed only where there is something: the banner, or the Center's
  /// column while it is open; a press outside it arrives from Ossido (#149), so the
  /// windows below keep their hover. Without that support the Center covers the screen.
  void _updateInput() {
    if (_centerOpen) {
      LayerControl.keyboard('exclusive');
      final size = _size;
      if (size == null) {
        LayerControl.inputRegion(null);
        return;
      }
      final left = size.width - columnMargin - columnWidth - 8;
      final column = Rect.fromLTWH(
        left,
        barHeight,
        size.width - left,
        size.height - barHeight,
      );
      LayerControl.transient(
        rects: [column],
        policy: TransientPolicy.panel,
      ).then((supported) {
        if (mounted && _centerOpen) {
          LayerControl.inputRegion(supported ? [column] : null);
        }
      });
      return;
    }
    LayerControl.clearTransient();
    LayerControl.keyboard('none');
    final size = _size;
    if (_banners.isEmpty || size == null) {
      LayerControl.inputRegion(const []);
      return;
    }
    // The column at the top right, with room for the close button.
    final left = size.width - columnMargin - columnWidth - 8;
    LayerControl.inputRegion([
      Rect.fromLTWH(left, barHeight, size.width - left, 260),
    ]);
  }

  /// A click: the app's `default` action, otherwise the sender app opens (N-02: the click
  /// launched the sender with its window).
  void _activate(ShellNotification n) {
    if (n.hasDefault) {
      widget.source.invoke(n.id, 'default');
    } else {
      // As after the `default` action the notification is handled and leaves the Center.
      widget.openApp(n);
      if (!n.resident) widget.source.dismiss(n.id);
    }
    // Clicked in the Center: the app comes forward and the Center closes.
    _closeCenter();
  }

  /// Rightward offset of the Center and opacity in the current phase; null if closed.
  (double, double)? _centerPose() {
    final ms = _center.value * (_center.duration?.inMilliseconds ?? 0);
    if (_centerOpen) {
      final x = centerOpenOffset(_center.isCompleted ? 1e9 : ms);
      return (x, 1 - x / centerDistance);
    }
    if (!_center.isAnimating) return null;
    final x = centerCloseOffset(ms);
    return (x, 1 - x / centerDistance);
  }

  @override
  Widget build(BuildContext context) {
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (mounted && !_centerOpen && _banners.isNotEmpty) _updateInput();
    });
    final pose = _centerPose();
    return LayoutBuilder(
      builder: (context, constraints) {
        _size = constraints.biggest;
        final width = constraints.maxWidth;
        final left = width - columnMargin - columnWidth;
        return Stack(
          clipBehavior: Clip.none,
          children: [
            if (pose != null)
              Positioned.fill(
                child: Opacity(
                  opacity: pose.$2.clamp(0.0, 1.0),
                  child: Transform.translate(
                    offset: Offset(pose.$1, 0),
                    child: _centerView(left),
                  ),
                ),
              ),
            if (!_centerOpen)
              for (final banner in _banners)
                Positioned(
                  key: banner.key,
                  left: left,
                  top: columnTop,
                  width: columnWidth,
                  child: BannerSlot(
                    notification: banner.notification,
                    leaving: banner.leaving,
                    persistent: banner.alert,
                    onGone: () => _gone(banner),
                    onActivate: () => _activate(banner.notification),
                    onDismiss: () =>
                        widget.source.dismiss(banner.notification.id),
                    onAction: (key) =>
                        widget.source.invoke(banner.notification.id, key),
                    onReply: (text) =>
                        widget.source.reply(banner.notification.id, text),
                    onReplyFocus: (focused) =>
                        LayerControl.keyboard(focused ? 'exclusive' : 'none'),
                  ),
                ),
          ],
        );
      },
    );
  }

  Widget _centerView(double left) {
    final groups = groupNotifications(widget.source.notifications.values);
    return Focus(
      focusNode: _focus,
      onKeyEvent: (node, event) {
        if (event is KeyDownEvent &&
            event.logicalKey == LogicalKeyboardKey.escape) {
          _closeCenter();
          return KeyEventResult.handled;
        }
        return KeyEventResult.ignored;
      },
      child: Stack(
        children: [
          Positioned.fill(
            child: GestureDetector(
              // With the clear menu open its own clicks (and the outside close) belong to the menu.
              onTap: _clearMenu ? null : _closeCenter,
              behavior: HitTestBehavior.opaque,
            ),
          ),
          Positioned(
            left: left,
            top: 0,
            bottom: 0,
            width: columnWidth,
            child: SingleChildScrollView(
              clipBehavior: Clip.none,
              padding: const EdgeInsets.only(top: columnTop, bottom: 60),
              child: _Scrim(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.stretch,
                  children: _column(groups),
                ),
              ),
            ),
          ),
          if (_clearMenuView() case final menu?) Positioned.fill(child: menu),
        ],
      ),
    );
  }

  /// The column (notifications.md › Notification Center · layout): the most recent group
  /// sits above the header, 15.5 pt over it; the others follow 7.5 pt below the header; the
  /// "Choose Widgets" pill and the clear menu close the column 16 pt below the last card.
  /// Empty: only the "no recent notifications" text 30 pt below the column top, and the pill
  /// 33 pt under it (N-03).
  List<Widget> _column(List<NotificationGroup> groups) {
    if (groups.isEmpty) {
      return [
        const SizedBox(height: emptyTextTop),
        _empty(),
        const SizedBox(height: emptyPillGap),
        Center(child: _widgetsPill()),
      ];
    }
    List<Widget> trimmed(List<Widget> items) =>
        items.isNotEmpty && items.last is SizedBox
        ? items.sublist(0, items.length - 1)
        : items;
    final rest = [for (final group in groups.skip(1)) ..._group(group)];
    return [
      ...trimmed(_group(groups.first)),
      const SizedBox(height: headerGapAbove),
      _header(true),
      const SizedBox(height: headerGapBelow),
      ...trimmed(rest),
      const SizedBox(height: footerGap),
      Center(
        child: Row(
          mainAxisSize: MainAxisSize.min,
          children: [
            _widgetsPill(),
            const SizedBox(width: footerSpacing),
            _clearMenuButton(),
          ],
        ),
      ),
    ];
  }

  /// "Choose Widgets": the pill that edits the widgets (91 × 24, SH-NC-13). Silica has no
  /// widgets and no widget gallery yet (silica-desktop#68, #70): the pill is shown as on the
  /// reference but disabled, never a control that does nothing.
  Widget _widgetsPill() => Opacity(
    key: const ValueKey('choose widgets'),
    opacity: disabledFooterOpacity,
    child: Semantics(
      enabled: false,
      button: true,
      child: FooterPill(
        size: widgetsPillSize,
        label: l10nOf(context).chooseWidgets,
      ),
    ),
  );

  /// The 24 pt circle next to the pill: the menu to clear the notifications.
  Widget _clearMenuButton() => GestureDetector(
    key: _clearMenuKey,
    onTap: () => setState(() => _clearMenu = !_clearMenu),
    child: const FooterPill(size: Size.square(clearMenuSize), ellipsis: true),
  );

  /// The clear menu, below the circle (items provisional: the reference menu was not opened).
  Widget? _clearMenuView() {
    if (!_clearMenu) return null;
    final box = _clearMenuKey.currentContext?.findRenderObject();
    final root = context.findRenderObject();
    if (box is! RenderBox || root is! RenderBox) return null;
    final at = box.localToGlobal(
      Offset(0, box.size.height + 4),
      ancestor: root,
    );
    return MenuHost(
      key: const ValueKey('clear menu'),
      entries: [
        MenuAction(
          l10nOf(context).clearAllNotifications,
          onSelected: () {
            setState(() => _clearMenu = false);
            widget.source.clear();
          },
        ),
      ],
      anchor: at,
      top: barHeight,
      onDismissed: () => setState(() => _clearMenu = false),
    );
  }

  Widget _header(bool clearable) => SizedBox(
    height: headerHeight,
    child: Row(
      children: [
        const SizedBox(width: 1),
        Expanded(
          child: Text(
            l10nOf(context).notificationsTitle,
            style: _style(headerSize, headerColor, weight: FontWeight.w700),
          ),
        ),
        if (clearable)
          GestureDetector(
            onTap: widget.source.clear,
            child: const CloseCircle(size: 20),
          ),
      ],
    ),
  );

  /// Empty state: white semibold text, centered on the column.
  Widget _empty() => SizedBox(
    height: emptyTextHeight,
    child: Center(
      child: Text(
        l10nOf(context).noNotifications,
        style: _style(fontSize, headerColor, weight: FontWeight.w600),
      ),
    ),
  );

  List<Widget> _group(NotificationGroup group) {
    final expanded = _expanded.contains(group.key) || group.items.length == 1;
    if (!expanded) {
      final strips = (group.items.length - 1).clamp(0, stackInsets.length);
      return [
        GestureDetector(
          onTap: () => setState(() => _expanded.add(group.key)),
          child: Stack(
            clipBehavior: Clip.none,
            children: [
              Padding(
                padding: EdgeInsets.only(bottom: strips * stackStep),
                child: _CenterCard(
                  notification: group.latest,
                  source: widget.source,
                  onTap: () => setState(() => _expanded.add(group.key)),
                ),
              ),
              for (var i = 0; i < strips; i++)
                Positioned(
                  left: stackInsets[i],
                  right: stackInsets[i],
                  bottom: (strips - 1 - i) * stackStep,
                  height: stackStrip,
                  child: const _StackStrip(),
                ),
            ],
          ),
        ),
        const SizedBox(height: cardGap),
      ];
    }
    return [
      if (group.items.length > 1)
        _GroupHeader(
          title: group.latest.appName,
          onCollapse: () => setState(() => _expanded.remove(group.key)),
          onClear: () {
            for (final n in group.items) {
              widget.source.dismiss(n.id);
            }
          },
        ),
      for (final n in group.items) ...[
        _CenterCard(
          notification: n,
          source: widget.source,
          onTap: () => _activate(n),
        ),
        const SizedBox(height: expandedGap),
      ],
    ];
  }
}

/// The header of an expanded group: the app name on the left (19 pt), on the right the
/// capsule to collapse the group (102 × 20) and the circle to clear it (20 × 20). With the
/// pointer on the circle the two shapes swap widths: the circle becomes the "Clear"
/// capsule (65 × 20) and the capsule a circle with an up arrow.
class _GroupHeader extends StatefulWidget {
  const _GroupHeader({
    required this.title,
    required this.onCollapse,
    required this.onClear,
  });

  final String title;
  final VoidCallback onCollapse;
  final VoidCallback onClear;

  @override
  State<_GroupHeader> createState() => _GroupHeaderState();
}

class _GroupHeaderState extends State<_GroupHeader> {
  bool _clearHover = false;

  Widget _pill({
    required double width,
    String? label,
    required VoidCallback onTap,
  }) => GestureDetector(
    onTap: onTap,
    child: AnimatedContainer(
      duration: groupMorph,
      curve: Curves.easeOut,
      width: width,
      height: groupPillHeight,
      alignment: Alignment.center,
      decoration: BoxDecoration(
        color: NotificationColors.of(context).pillFill,
        borderRadius: const BorderRadius.all(
          Radius.circular(groupPillHeight / 2),
        ),
      ),
      // No label: the × circle, drawn centred (a typed × is off-centre, #166).
      child: label == null
          ? CustomPaint(
              key: const ValueKey('group clear glyph'),
              size: const Size.square(groupPillHeight),
              painter: CrossGlyph(
                NotificationColors.of(context).pillText,
                arm: 3.2,
                stroke: 1.6,
              ),
            )
          : Text(
              label,
              maxLines: 1,
              overflow: TextOverflow.clip,
              style: _style(
                smallSize,
                NotificationColors.of(context).pillText,
                weight: FontWeight.w600,
              ),
            ),
    ),
  );

  @override
  Widget build(BuildContext context) => SizedBox(
    height: groupHeaderHeight,
    child: Row(
      children: [
        const SizedBox(width: 1),
        Expanded(
          child: Text(
            widget.title,
            maxLines: 1,
            overflow: TextOverflow.ellipsis,
            style: _style(groupTitleSize, headerColor, weight: FontWeight.w700),
          ),
        ),
        _pill(
          width: _clearHover ? groupPillHeight : groupCollapseWidth,
          label: _clearHover ? '↑' : l10nOf(context).collapse,
          onTap: widget.onCollapse,
        ),
        const SizedBox(width: 8),
        MouseRegion(
          onEnter: (_) => setState(() => _clearHover = true),
          onExit: (_) => setState(() => _clearHover = false),
          child: _pill(
            width: _clearHover ? groupClearWidth : groupPillHeight,
            label: _clearHover ? l10nOf(context).clear : null,
            onTap: widget.onClear,
          ),
        ),
      ],
    ),
  );
}

/// A Center card: with the time, the actions and the close button on hover.
class _CenterCard extends StatefulWidget {
  const _CenterCard({
    required this.notification,
    required this.source,
    required this.onTap,
  });

  final ShellNotification notification;
  final NotificationsSource source;
  final VoidCallback onTap;

  @override
  State<_CenterCard> createState() => _CenterCardState();
}

/// A card in the Center. Dragged to the right it follows the pointer and, on release, leaves
/// with the banner's exit and is cleared (SH-NC-12; the Center drag was not measured: the
/// banner's N-02 behaviour, provisional).
class _CenterCardState extends State<_CenterCard>
    with SingleTickerProviderStateMixin {
  bool _hover = false;
  double _drag = 0;
  late final AnimationController _exit = AnimationController(
    vsync: this,
    duration: bannerExitDuration,
  )..addListener(() => setState(() {}));

  @override
  void dispose() {
    _exit.dispose();
    super.dispose();
  }

  Future<void> _leave() async {
    if (_exit.isAnimating || _exit.value > 0) return;
    await _exit.forward(from: 0);
    if (mounted) widget.source.dismiss(widget.notification.id);
  }

  @override
  Widget build(BuildContext context) {
    final n = widget.notification;
    final exitMs = _exit.value * bannerExitDuration.inMilliseconds;
    final exiting = _exit.value > 0;
    final dx = _drag + (exiting ? bannerExitOffset(exitMs) : 0);
    final opacity = exiting ? bannerExitOpacity(exitMs) : 1.0;
    return Transform.translate(
      offset: Offset(dx, 0),
      child: Opacity(
        opacity: opacity.clamp(0.0, 1.0),
        child: MouseRegion(
          onEnter: (_) => setState(() => _hover = true),
          onExit: (_) => setState(() => _hover = false),
          child: GestureDetector(
            onTap: exiting ? null : widget.onTap,
            onHorizontalDragUpdate: exiting
                ? null
                : (d) => setState(
                    () => _drag = (_drag + d.delta.dx).clamp(0, 1e4),
                  ),
            // Released after a drag to the right: it leaves; a drag back to the start stays.
            onHorizontalDragEnd: exiting
                ? null
                : (_) => _drag > 0 ? _leave() : null,
            child: NotificationCard(
              notification: n,
              showTime: true,
              showActions: true,
              bodyLines: 4,
              showClose: _hover && !exiting,
              onClose: () => widget.source.dismiss(n.id),
              onAction: (key) => widget.source.invoke(n.id, key),
              onReply: (text) => widget.source.reply(n.id, text),
            ),
          ),
        ),
      ),
    );
  }
}

/// The card of a notification: glass, icon, title, body; in the Center also time and actions.
class NotificationCard extends StatelessWidget {
  const NotificationCard({
    super.key,
    required this.notification,
    this.showTime = false,
    this.showActions = false,
    this.bodyLines = 2,
    this.shadow = false,
    this.showClose = false,
    this.onClose,
    this.onAction,
    this.showDetails = false,
    this.onDetails,
    this.onReply,
    this.onReplyFocus,
  });

  final ShellNotification notification;

  /// The reply field, for notifications that take one (with the actions shown).
  final ValueChanged<String>? onReply;

  /// The reply field takes or loses the keyboard (the banner layer asks for it).
  final ValueChanged<bool>? onReplyFocus;
  final bool showTime;
  final bool showActions;
  final int bodyLines;
  final bool shadow;
  final bool showClose;
  final VoidCallback? onClose;
  final ValueChanged<String>? onAction;

  /// The "Details" capsule on the right (banner on hover): the text shortens to make room
  /// for it.
  final bool showDetails;
  final VoidCallback? onDetails;

  @override
  Widget build(BuildContext context) {
    final n = notification;
    final c = NotificationColors.of(context);
    final buttons = showActions ? n.buttons : const <(String, String)>[];
    final card = Stack(
      clipBehavior: Clip.none,
      children: [
        if (shadow)
          const Positioned.fill(
            child: CachedShadow(
              cacheKey: 'notification card',
              bleed: 60,
              paintShadow: _paintOutsideShadow,
            ),
          ),
        // The dark border line sits 1 pt outside the frame: the glass is 1 pt wider per
        // side and Ossido draws the line there.
        const Positioned(
          left: -1,
          top: -1,
          right: -1,
          bottom: -1,
          child: Glass(
            radius: cardRadius + 1,
            material: GlassMaterial.notification,
          ),
        ),
        ClipRRect(
          borderRadius: const BorderRadius.all(Radius.circular(cardRadius)),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              _content(c, n),
              if (showActions && n.canReply && onReply != null)
                _ReplyField(
                  key: ValueKey('reply ${n.id}'),
                  placeholder: n.replyLabel,
                  colors: c,
                  onSubmit: onReply!,
                  onFocus: onReplyFocus,
                ),
              if (buttons.isNotEmpty) _actions(c, buttons),
            ],
          ),
        ),
      ],
    );
    return Stack(
      clipBehavior: Clip.none,
      children: [
        card,
        // The × on hover: a 21 pt circle centered 4 pt right of and 6 pt below the top left
        // corner of the card.
        if (showClose)
          Positioned(
            left: closeCenter.dx - closeSize / 2,
            top: closeCenter.dy - closeSize / 2,
            child: GestureDetector(
              onTap: onClose,
              child: const CloseCircle(size: closeSize),
            ),
          ),
        if (showDetails)
          Positioned(
            right: detailsInset,
            top: 0,
            bottom: 0,
            child: Center(
              child: GestureDetector(
                onTap: onDetails,
                child: Container(
                  width: detailsSize.width,
                  height: detailsSize.height,
                  alignment: Alignment.center,
                  decoration: BoxDecoration(
                    color: c.buttonFill,
                    borderRadius: const BorderRadius.all(Radius.circular(10.5)),
                  ),
                  child: Text(
                    l10nOf(context).details,
                    style: _style(smallSize, c.buttonText),
                  ),
                ),
              ),
            ),
          ),
      ],
    );
  }

  Widget _content(NotificationColors c, ShellNotification n) {
    final title = _style(fontSize, c.text, weight: FontWeight.w600);
    final body = _style(fontSize, c.text);
    return Padding(
      padding: EdgeInsets.fromLTRB(
        iconLeft,
        paddingTop,
        showDetails
            ? detailsInset + detailsSize.width + detailsGap
            : paddingRight,
        paddingBottom,
      ),
      child: Row(
        children: [
          SizedBox(width: iconSize, height: iconSize, child: AppIcon(n)),
          const SizedBox(width: textLeft - iconLeft - iconSize),
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Row(
                  crossAxisAlignment: CrossAxisAlignment.baseline,
                  textBaseline: TextBaseline.alphabetic,
                  children: [
                    Expanded(
                      child: Text(
                        n.summary.isEmpty ? n.appName : n.summary,
                        maxLines: 1,
                        overflow: TextOverflow.ellipsis,
                        style: title,
                      ),
                    ),
                    if (showTime) ...[
                      const SizedBox(width: 8),
                      Text(
                        timeLabel(n.timestamp),
                        style: _style(smallSize, c.secondary),
                      ),
                    ],
                  ],
                ),
                if (n.body.isNotEmpty)
                  Text(
                    n.body,
                    maxLines: bodyLines,
                    overflow: TextOverflow.ellipsis,
                    style: body,
                  ),
              ],
            ),
          ),
        ],
      ),
    );
  }

  Widget _actions(NotificationColors c, List<(String, String)> buttons) {
    return DecoratedBox(
      decoration: BoxDecoration(
        color: c.actionsFill,
        border: Border(top: BorderSide(color: c.actionsDivider)),
      ),
      child: Padding(
        padding: const EdgeInsets.all(actionsInset),
        child: LayoutBuilder(
          builder: (context, constraints) {
            final width = (constraints.maxWidth - 8) / 2;
            return Wrap(
              spacing: 8,
              runSpacing: 8,
              children: [
                for (final (key, label) in buttons)
                  GestureDetector(
                    onTap: () => onAction?.call(key),
                    child: Container(
                      width: buttons.length == 1 ? constraints.maxWidth : width,
                      height: buttonHeight,
                      alignment: Alignment.center,
                      decoration: BoxDecoration(
                        color: c.buttonFill,
                        borderRadius: const BorderRadius.all(
                          Radius.circular(buttonHeight / 2),
                        ),
                      ),
                      child: Text(
                        label,
                        maxLines: 1,
                        overflow: TextOverflow.ellipsis,
                        style: _style(fontSize, c.buttonText),
                      ),
                    ),
                  ),
              ],
            );
          },
        ),
      ),
    );
  }
}

/// The card's shadow, outside the card only.
void _paintOutsideShadow(Canvas canvas, Size size) {
  final card = RRect.fromRectAndRadius(
    Offset.zero & size,
    const Radius.circular(cardRadius),
  );
  canvas
    ..save()
    ..clipPath(
      Path.combine(
        PathOperation.difference,
        Path()..addRect((Offset.zero & size).inflate(60)),
        Path()..addRRect(card),
      ),
    )
    ..drawRRect(
      card.shift(shadowOffset),
      Paint()
        ..color = shadowColor
        ..maskFilter = const MaskFilter.blur(BlurStyle.normal, shadowSigma),
    )
    ..restore();
}

/// The app icon: the notification image, the requested icon, the one of the desktop
/// entry, otherwise a tile with the initial.
class AppIcon extends StatelessWidget {
  const AppIcon(this.notification, {super.key});

  final ShellNotification notification;

  @override
  Widget build(BuildContext context) {
    final n = notification;
    final path =
        findIcon(n.imagePath.isNotEmpty ? n.imagePath : null) ??
        findIcon(n.appIcon) ??
        findIcon(n.desktopEntry);
    if (path != null) {
      return path.endsWith('.svg')
          ? SvgPicture.file(File(path))
          : Image.file(File(path));
    }
    return CustomPaint(painter: _InitialIcon(n.appName));
  }
}

class _InitialIcon extends CustomPainter {
  const _InitialIcon(this.name);

  final String name;

  @override
  void paint(Canvas canvas, Size size) {
    final rect = Offset.zero & size;
    // Corners of ~7 pt over 32, a 1 px darker outline (notifications.md).
    final shape = RRect.fromRectAndRadius(rect, const Radius.circular(7));
    canvas.drawRRect(shape, Paint()..color = const Color(0xFFE8E8EC));
    canvas.drawRRect(
      shape.deflate(0.5),
      Paint()
        ..style = PaintingStyle.stroke
        ..color = const Color(0xFFBABBBF),
    );
    final initial = name.isEmpty ? '' : name.characters.first.toUpperCase();
    final text = TextPainter(
      text: TextSpan(
        text: initial,
        style: _style(16, const Color(0xFF55565A), weight: FontWeight.w600),
      ),
      textDirection: TextDirection.ltr,
    )..layout();
    text.paint(canvas, rect.center - Offset(text.width / 2, text.height / 2));
  }

  @override
  bool shouldRepaint(_InitialIcon old) => old.name != name;
}

/// The circle with the × (close, clear all): background `#b4b5b9`, 1 pt dark rim,
/// 2 px glyph `#5a5b5c`.
/// The reply field of a notification that takes one (inline-reply; the reference reply was
/// not tried: the field sits in the actions section with its colours, provisional). Return
/// sends the text; an empty field sends nothing.
class _ReplyField extends StatefulWidget {
  const _ReplyField({
    super.key,
    required this.placeholder,
    required this.colors,
    required this.onSubmit,
    this.onFocus,
  });

  final String placeholder;
  final NotificationColors colors;
  final ValueChanged<String> onSubmit;
  final ValueChanged<bool>? onFocus;

  @override
  State<_ReplyField> createState() => _ReplyFieldState();
}

class _ReplyFieldState extends State<_ReplyField> {
  final TextEditingController _text = TextEditingController();
  final FocusNode _focus = FocusNode();

  @override
  void initState() {
    super.initState();
    _focus.addListener(() => widget.onFocus?.call(_focus.hasFocus));
  }

  @override
  void dispose() {
    _text.dispose();
    _focus.dispose();
    super.dispose();
  }

  void _send(String text) {
    final trimmed = text.trim();
    if (trimmed.isEmpty) return;
    widget.onSubmit(trimmed);
    _text.clear();
  }

  @override
  Widget build(BuildContext context) {
    final c = widget.colors;
    return DecoratedBox(
      decoration: BoxDecoration(
        color: c.actionsFill,
        border: Border(top: BorderSide(color: c.actionsDivider)),
      ),
      child: Padding(
        padding: const EdgeInsets.all(actionsInset),
        child: SilicaTextField(
          controller: _text,
          focusNode: _focus,
          placeholder: widget.placeholder.isEmpty
              ? l10nOf(context).replyPlaceholder
              : widget.placeholder,
          onSubmitted: _send,
        ),
      ),
    );
  }
}

/// The footer controls (notifications.md › layout): a pill or a circle on the same material
/// as the × circle — fill `#b4b5b9` light, bright row on top and bottom, dark side line.
class FooterPill extends StatelessWidget {
  const FooterPill({
    super.key,
    required this.size,
    this.label,
    this.ellipsis = false,
  });

  final Size size;
  final String? label;

  /// The "…" glyph of the menu circle.
  final bool ellipsis;

  @override
  Widget build(BuildContext context) {
    final c = NotificationColors.of(context);
    return Container(
      width: size.width,
      height: size.height,
      alignment: Alignment.center,
      decoration: BoxDecoration(
        color: c.pillFill,
        border: Border.all(color: c.closeBorder),
        borderRadius: BorderRadius.circular(size.height / 2),
      ),
      child: Text(
        ellipsis ? '\u00b7\u00b7\u00b7' : label ?? '',
        maxLines: 1,
        style: _style(widgetsPillTextSize, c.pillText),
      ),
    );
  }
}

class CloseCircle extends StatelessWidget {
  const CloseCircle({super.key, required this.size});

  final double size;

  @override
  Widget build(BuildContext context) => CustomPaint(
    size: Size.square(size),
    painter: _ClosePainter(NotificationColors.of(context)),
  );
}

class _ClosePainter extends CustomPainter {
  const _ClosePainter(this.colors);

  final NotificationColors colors;

  @override
  void paint(Canvas canvas, Size size) {
    final center = size.center(Offset.zero);
    final radius = size.width / 2;
    canvas.drawCircle(center, radius, Paint()..color = colors.closeBorder);
    canvas.drawCircle(center, radius - 1, Paint()..color = colors.closeFill);
    CrossGlyph(colors.closeGlyph, arm: radius * 0.32).paint(canvas, size);
  }

  @override
  bool shouldRepaint(_ClosePainter old) => old.colors != colors;
}

/// A card peeking out under a collapsed group.
class _StackStrip extends StatelessWidget {
  const _StackStrip();

  @override
  Widget build(BuildContext context) => ClipRRect(
    borderRadius: const BorderRadius.vertical(bottom: Radius.circular(10)),
    child: Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Expanded(
          child: Container(color: NotificationColors.of(context).stripFill),
        ),
        // A lighter line at the bottom (#bc–#be against #a7–#af).
        Container(height: 1, color: NotificationColors.of(context).stripLine),
      ],
    ),
  );
}

/// The Center scrim: a soft dark halo around the whole column (not per card), fading in
/// ~60 pt.
class _Scrim extends StatelessWidget {
  const _Scrim({required this.child});

  final Widget child;

  @override
  Widget build(BuildContext context) => DecoratedBox(
    decoration: const BoxDecoration(
      boxShadow: [BoxShadow(color: scrimColor, blurRadius: scrimBlur)],
    ),
    child: child,
  );
}
