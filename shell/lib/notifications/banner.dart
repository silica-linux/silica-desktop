// A notification banner, from entering to leaving (plan/reference/notifications.md,
// "Behaviors", session 8):
// - it slides in from the right (~400 ms), fading in (~210 ms) and blurred, sharpening as it
//   slows down;
// - it stays ~5 s from appearing; with the pointer over it it does not expire, and it leaves
//   ~1.1 s after the pointer exits; critical notifications stay;
// - on hover the × appears on the top left corner and the "Details" capsule on the right
//   (the text shortens);
// - dragged to the right it follows the pointer 1:1 and on release it leaves anyway, without
//   opening the app; a click opens the app (`default` action);
// - it leaves to the right in ~190 ms, fading out halfway and blurring. When a new
//   notification arrives the current banner leaves while the new one enters.

import 'dart:async';
import 'dart:ui' show ImageFilter;

import 'package:flutter/widgets.dart';

import 'model.dart';
import 'notifications.dart';

class BannerSlot extends StatefulWidget {
  const BannerSlot({
    super.key,
    required this.notification,
    required this.leaving,
    required this.onGone,
    required this.onActivate,
    required this.onDismiss,
    this.onHover,
  });

  final ShellNotification notification;

  /// Replaced by a more recent banner: it leaves at once.
  final bool leaving;

  /// The exit is over: the banner is removed.
  final VoidCallback onGone;

  /// Click on the banner.
  final VoidCallback onActivate;

  /// Click on the ×.
  final VoidCallback onDismiss;

  /// The pointer enters or exits (for the input region).
  final ValueChanged<bool>? onHover;

  @override
  State<BannerSlot> createState() => BannerSlotState();
}

class BannerSlotState extends State<BannerSlot> with TickerProviderStateMixin {
  late final AnimationController _enter;
  late final AnimationController _exit;
  Timer? _timer;
  bool _hover = false;
  bool _expanded = false;
  double _drag = 0;

  bool get leaving => _exit.isAnimating || _exit.value > 0;

  @override
  void initState() {
    super.initState();
    void repaint() => setState(() {});
    _enter = AnimationController(vsync: this, duration: bannerEnterDuration)
      ..addListener(repaint)
      ..forward();
    _exit = AnimationController(vsync: this, duration: bannerExitDuration)
      ..addListener(repaint);
    if (widget.leaving) {
      _leave();
    } else {
      _schedule(bannerDuration);
    }
  }

  @override
  void didUpdateWidget(BannerSlot old) {
    super.didUpdateWidget(old);
    if (widget.leaving && !old.leaving) _leave();
  }

  @override
  void dispose() {
    _timer?.cancel();
    _enter.dispose();
    _exit.dispose();
    super.dispose();
  }

  void _schedule(Duration after) {
    _timer?.cancel();
    if (widget.notification.critical || _hover) return;
    _timer = Timer(after, _leave);
  }

  /// Leaves to the right from where it is (even when dragged).
  void leave() => _leave();

  Future<void> _leave() async {
    if (leaving) return;
    _timer?.cancel();
    await _exit.forward(from: 0);
    if (mounted) widget.onGone();
  }

  void _setHover(bool hover) {
    if (leaving) return;
    setState(() => _hover = hover);
    widget.onHover?.call(hover);
    if (hover) {
      _timer?.cancel();
    } else {
      _schedule(bannerLinger);
    }
  }

  @override
  Widget build(BuildContext context) {
    final enterMs = _enter.value * bannerEnterDuration.inMilliseconds;
    final exitMs = _exit.value * bannerExitDuration.inMilliseconds;
    final exiting = _exit.value > 0;
    final dx =
        bannerEnterOffset(_enter.isCompleted ? 1e9 : enterMs) +
        _drag +
        (exiting ? bannerExitOffset(exitMs) : 0);
    final opacity =
        bannerEnterOpacity(enterMs) * (exiting ? bannerExitOpacity(exitMs) : 1);
    final blur = exiting
        ? bannerExitBlur(exitMs)
        : bannerEnterBlur(_enter.isCompleted ? 1e9 : enterMs);
    Widget card = NotificationCard(
      notification: widget.notification,
      shadow: true,
      showClose: _hover && !exiting,
      onClose: widget.onDismiss,
      showDetails: _hover && !exiting && !_expanded,
      onDetails: () => setState(() {
        _expanded = true;
        _timer?.cancel();
      }),
      showActions: _expanded,
      bodyLines: _expanded ? 6 : 2,
    );
    if (blur > 0.05) {
      card = ImageFiltered(
        imageFilter: ImageFilter.blur(sigmaX: blur, sigmaY: blur),
        child: card,
      );
    }
    return Transform.translate(
      offset: Offset(dx, 0),
      child: Opacity(
        opacity: opacity.clamp(0.0, 1.0),
        child: MouseRegion(
          onEnter: (_) => _setHover(true),
          onExit: (_) => _setHover(false),
          child: GestureDetector(
            behavior: HitTestBehavior.opaque,
            onTap: exiting
                ? null
                : () {
                    widget.onActivate();
                    _leave();
                  },
            onHorizontalDragUpdate: exiting
                ? null
                : (d) => setState(
                    () => _drag = (_drag + d.delta.dx).clamp(0, 1e4),
                  ),
            onHorizontalDragEnd: exiting ? null : (_) => _leave(),
            child: card,
          ),
        ),
      ),
    );
  }
}
