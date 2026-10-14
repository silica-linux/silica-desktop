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
import '../system/icon_lookup.dart';
import '../system/layer_control.dart';
import '../system/shell_channel.dart';
import '../system/system_status.dart';
import 'banner.dart';
import 'model.dart';
import 'service.dart';

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

/// The card colors, light and dark. Dark is measured on
/// `shots/glass/notifications/gray-dark.png` (Center open over grey, lightest pixels of the
/// texts, backgrounds in clean spots) and on `banner-gray-dark.png` (text `#eaeaeb`):
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

/// `HH:MM` of the arrival time, as in the reference Center.
String timeLabel(DateTime t) =>
    '${t.hour.toString().padLeft(2, '0')}:${t.minute.toString().padLeft(2, '0')}';

class NotificationsLayer extends StatefulWidget {
  const NotificationsLayer({
    super.key,
    required this.source,
    this.system,
    this.startOpen = false,
  });

  final NotificationsSource source;

  /// With Do Not Disturb on, notifications go to the Center without a banner.
  final SystemSource? system;
  final bool startOpen;

  @override
  State<NotificationsLayer> createState() => _NotificationsLayerState();
}

class _BannerEntry {
  _BannerEntry(this.notification);

  final Key key = UniqueKey();
  ShellNotification notification;
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
    if (_centerOpen || (widget.system?.value.doNotDisturb ?? false)) return;
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
      _banners.add(_BannerEntry(n));
    });
    _updateInput();
  }

  void _gone(_BannerEntry banner) {
    setState(() => _banners.remove(banner));
    _updateInput();
  }

  void _toggleCenter() => _centerOpen ? _closeCenter() : _openCenter();

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
    });
    _center
      ..duration = centerCloseDuration
      ..forward(from: 0);
    _updateInput();
  }

  /// The pointer is needed only where there is something: the banner, or the whole screen
  /// with the Center open (a click outside closes it).
  void _updateInput() {
    if (_centerOpen) {
      LayerControl.inputRegion(null);
      LayerControl.keyboard('exclusive');
      return;
    }
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

  void _activate(ShellNotification n) {
    if (n.hasDefault) widget.source.invoke(n.id, 'default');
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
                    onGone: () => _gone(banner),
                    onActivate: () => _activate(banner.notification),
                    onDismiss: () =>
                        widget.source.dismiss(banner.notification.id),
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
              onTap: _closeCenter,
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
                  children: [
                    _header(groups.isNotEmpty),
                    const SizedBox(height: headerGapBelow),
                    if (groups.isEmpty) _empty(),
                    for (final group in groups) ..._group(group),
                  ],
                ),
              ),
            ),
          ),
        ],
      ),
    );
  }

  Widget _header(bool clearable) => SizedBox(
    height: headerHeight,
    child: Row(
      children: [
        const SizedBox(width: 1),
        Expanded(
          child: Text(
            'Notifiche',
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
  Widget _empty() => Padding(
    padding: const EdgeInsets.symmetric(vertical: 24),
    child: Center(
      child: Text(
        'Nessuna notifica',
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
/// pointer on the circle the two shapes swap widths: the circle becomes the "Cancella"
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
    required String label,
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
      child: Text(
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
          label: _clearHover ? '↑' : 'Riduci',
          onTap: widget.onCollapse,
        ),
        const SizedBox(width: 8),
        MouseRegion(
          onEnter: (_) => setState(() => _clearHover = true),
          onExit: (_) => setState(() => _clearHover = false),
          child: _pill(
            width: _clearHover ? groupClearWidth : groupPillHeight,
            label: _clearHover ? 'Cancella' : '×',
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

class _CenterCardState extends State<_CenterCard> {
  bool _hover = false;

  @override
  Widget build(BuildContext context) {
    final n = widget.notification;
    return MouseRegion(
      onEnter: (_) => setState(() => _hover = true),
      onExit: (_) => setState(() => _hover = false),
      child: GestureDetector(
        onTap: widget.onTap,
        child: NotificationCard(
          notification: n,
          showTime: true,
          showActions: true,
          bodyLines: 4,
          showClose: _hover,
          onClose: () => widget.source.dismiss(n.id),
          onAction: (key) => widget.source.invoke(n.id, key),
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
  });

  final ShellNotification notification;
  final bool showTime;
  final bool showActions;
  final int bodyLines;
  final bool shadow;
  final bool showClose;
  final VoidCallback? onClose;
  final ValueChanged<String>? onAction;

  /// The "Dettagli" capsule on the right (banner on hover): the text shortens to make room
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
          const Positioned.fill(child: CustomPaint(painter: _OutsideShadow())),
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
                    'Dettagli',
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

class _OutsideShadow extends CustomPainter {
  const _OutsideShadow();

  @override
  void paint(Canvas canvas, Size size) {
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

  @override
  bool shouldRepaint(_OutsideShadow old) => false;
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
    final arm = radius * 0.32;
    final stroke = Paint()
      ..color = colors.closeGlyph
      ..strokeWidth = 2
      ..strokeCap = StrokeCap.round;
    canvas.drawLine(
      center - Offset(arm, arm),
      center + Offset(arm, arm),
      stroke,
    );
    canvas.drawLine(
      center + Offset(-arm, arm),
      center + Offset(arm, -arm),
      stroke,
    );
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
