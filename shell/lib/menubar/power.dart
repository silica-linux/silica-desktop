// The power confirmations of the logo menu (SH-MB-05, docs/spec/misc.md): Restart…, Shut
// Down… and Log Out… open an alert card with a 60-second countdown; at zero, or with the
// default button, the action runs. With ⌥ the menu items act at once, without the card.
//
// Card (measured): 292 × 266 pt (282 with a two-line title), radius ~28, centred in the upper
// third of the screen, above everything; 66 pt gray icon at (+19, +19); bold 13 pt title at
// y +99 (250 pt wide); 13 pt body with the countdown at y +141; the checkbox row at y +189;
// two 128 × 30 capsules at y +221, gap 6 (gray cancel, accent default), 15 pt from the bottom.
// It zooms in with a small overshoot (~106 % at ~90 ms, 100 % by ~210 ms) while the content
// fades in; cancelling removes it in one frame. The texts are our own.
//
// The actions go to logind (org.freedesktop.login1) on the system bus, only in a real Silica
// session (XDG_SESSION_DESKTOP=silica): a nested or test session must never power off the
// machine it runs in.

import 'dart:async';
import 'dart:io';
import 'dart:math' as math;

import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';
import 'package:silica_ui/silica_ui.dart';

import '../l10n/l10n.dart';
import '../system/dbus.dart';

enum PowerAction { sleep, restart, shutDown, logOut }

/// True in a real Silica session, where the power actions reach logind.
bool powerAvailable([Map<String, String>? env]) =>
    (env ?? Platform.environment)['XDG_SESSION_DESKTOP'] == 'silica';

/// Runs `action` now: suspend, reboot and power off through logind; log out through
/// `logOut` (Ossido's `session.quit`).
Future<void> runPower(
  PowerAction action, {
  required VoidCallback logOut,
  DBus? dbus,
}) async {
  if (action == PowerAction.logOut) {
    logOut();
    return;
  }
  if (!powerAvailable()) return;
  final method = switch (action) {
    PowerAction.sleep => 'Suspend',
    PowerAction.restart => 'Reboot',
    PowerAction.shutDown => 'PowerOff',
    PowerAction.logOut => '',
  };
  try {
    await (dbus ?? DBus.instance).call(
      bus: DBusBus.system,
      destination: 'org.freedesktop.login1',
      path: '/org/freedesktop/login1',
      interface: 'org.freedesktop.login1.Manager',
      method: method,
      signature: 'b',
      // Interactive: polkit may ask for authentication.
      args: [true],
    );
  } on PlatformException {
    // Refused or not available: nothing happens, as when cancelled.
  }
}

const Size powerCardSize = Size(292, 266);
const double powerCardTallHeight = 282;
const double powerCardRadius = 28;
const int powerCountdown = 60;

@immutable
class PowerColors {
  const PowerColors({
    required this.fill,
    required this.edge,
    required this.text,
    required this.icon,
    required this.iconGlyph,
    required this.cancel,
    required this.cancelText,
    required this.accent,
  });

  final Color fill;
  final Color edge;
  final Color text;
  final Color icon;
  final Color iconGlyph;
  final Color cancel;
  final Color cancelText;
  final Color accent;

  static const PowerColors light = PowerColors(
    fill: Color(0xFFFEFEFE),
    edge: Color(0xFFADADAD),
    text: Color(0xFF1D1D1F),
    // Provisional: the icon disc; the glyph is the measured `#969899`.
    icon: Color(0xFFE3E3E5),
    iconGlyph: Color(0xFF969899),
    cancel: Color(0xFFEAEAEA),
    cancelText: Color(0xFF1D1D1F),
    accent: Color(0xFF296FFB),
  );

  static const PowerColors dark = PowerColors(
    fill: Color(0xFF3E3E3F),
    edge: Color(0xFF000000),
    text: Color(0xFFEDEDED),
    icon: Color(0xFF58585A),
    iconGlyph: Color(0xFFB4B5B6),
    cancel: Color(0xFF4C4C4D),
    cancelText: Color(0xFFEDEDED),
    accent: Color(0xFF296FFB),
  );

  static PowerColors of(BuildContext context) =>
      MediaQuery.maybePlatformBrightnessOf(context) == Brightness.dark
      ? dark
      : light;
}

/// The scale of the card `ms` after it appears: from 0.9 up to ~106 % at 90 ms, then 100 %
/// at 210 ms (damped, measured from the appearance frames).
double powerCardScale(double ms) {
  if (ms <= 0) return 0.9;
  if (ms < 90) return 0.9 + 0.16 * Curves.easeOut.transform(ms / 90);
  if (ms < 210)
    return 1.06 - 0.06 * Curves.easeInOut.transform((ms - 90) / 120);
  return 1;
}

/// The content's opacity during the growth (it fades in over the first ~150 ms).
double powerCardOpacity(double ms) => (ms / 150).clamp(0.0, 1.0);

/// The confirmation card for `action` (restart, shut down or log out).
class PowerConfirmation extends StatefulWidget {
  const PowerConfirmation({
    super.key,
    required this.action,
    required this.user,
    required this.onConfirm,
    required this.onCancel,
  });

  final PowerAction action;
  final String user;
  final VoidCallback onConfirm;
  final VoidCallback onCancel;

  @override
  State<PowerConfirmation> createState() => _PowerConfirmationState();
}

class _PowerConfirmationState extends State<PowerConfirmation>
    with SingleTickerProviderStateMixin {
  late final AnimationController _appear = AnimationController(
    vsync: this,
    duration: const Duration(milliseconds: 210),
  )..forward();
  late Timer _tick;
  int _left = powerCountdown;
  bool _reopen = false;
  final FocusNode _focus = FocusNode();

  @override
  void initState() {
    super.initState();
    _tick = Timer.periodic(const Duration(seconds: 1), (_) {
      if (!mounted) return;
      if (_left <= 1) {
        _tick.cancel();
        widget.onConfirm();
        return;
      }
      setState(() => _left--);
    });
    _focus.requestFocus();
  }

  @override
  void dispose() {
    _tick.cancel();
    _appear.dispose();
    _focus.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final l = l10nOf(context);
    final c = PowerColors.of(context);
    final (title, body, button) = switch (widget.action) {
      PowerAction.restart => (
        l.powerRestartTitle,
        l.powerRestartBody(_left),
        l.powerRestartButton,
      ),
      PowerAction.shutDown => (
        l.powerShutDownTitle,
        l.powerShutDownBody(_left),
        l.powerShutDownButton,
      ),
      _ => (
        l.powerLogOutTitle(widget.user),
        l.powerLogOutBody(_left),
        l.powerLogOutButton,
      ),
    };
    final height = widget.action == PowerAction.logOut
        ? powerCardTallHeight
        : powerCardSize.height;
    final regular = TextStyle(
      fontFamily: uiFontFamily,
      package: 'silica_ui',
      fontSize: 13,
      color: c.text,
      height: 16 / 13,
    );
    return KeyboardListener(
      focusNode: _focus,
      onKeyEvent: (e) {
        if (e is! KeyDownEvent) return;
        if (e.logicalKey == LogicalKeyboardKey.escape) widget.onCancel();
        if (e.logicalKey == LogicalKeyboardKey.enter) widget.onConfirm();
      },
      child: AnimatedBuilder(
        animation: _appear,
        builder: (context, child) {
          final ms = _appear.value * 210;
          return Transform.scale(
            scale: powerCardScale(ms),
            child: Opacity(opacity: powerCardOpacity(ms), child: child),
          );
        },
        child: Container(
          key: const ValueKey('power confirmation'),
          width: powerCardSize.width,
          height: height,
          decoration: BoxDecoration(
            color: c.fill,
            borderRadius: BorderRadius.circular(powerCardRadius),
            border: Border.all(color: c.edge, width: 0.5),
            boxShadow: const [
              BoxShadow(
                color: Color(0x40000000),
                blurRadius: 40,
                offset: Offset(0, 12),
              ),
            ],
          ),
          child: Stack(
            children: [
              Positioned(
                left: 19,
                top: 19,
                width: 66,
                height: 66,
                child: CustomPaint(
                  painter: _PowerIconPainter(
                    disc: c.icon,
                    glyph: c.iconGlyph,
                    person: widget.action == PowerAction.logOut,
                  ),
                ),
              ),
              Positioned(
                left: 21,
                top: 99,
                width: 250,
                child: Text(
                  title,
                  maxLines: 2,
                  style: regular.copyWith(fontWeight: FontWeight.w700),
                ),
              ),
              Positioned(
                left: 21,
                top: height - 125,
                width: 250,
                child: Text(body, style: regular),
              ),
              Positioned(
                left: 21,
                top: height - 77,
                height: 18,
                child: SilicaCheckbox(
                  value: _reopen,
                  label: l.powerReopenWindows,
                  onChanged: (v) => setState(() => _reopen = v),
                ),
              ),
              Positioned(
                left: 15,
                top: height - 45,
                child: _Capsule(
                  key: const ValueKey('power cancel'),
                  label: l.cancel,
                  fill: c.cancel,
                  color: c.cancelText,
                  onTap: widget.onCancel,
                ),
              ),
              Positioned(
                left: 149,
                top: height - 45,
                child: _Capsule(
                  key: const ValueKey('power confirm'),
                  label: button,
                  fill: c.accent,
                  color: const Color(0xFFFFFFFF),
                  onTap: widget.onConfirm,
                ),
              ),
            ],
          ),
        ),
      ),
    );
  }
}

class _Capsule extends StatelessWidget {
  const _Capsule({
    super.key,
    required this.label,
    required this.fill,
    required this.color,
    required this.onTap,
  });

  final String label;
  final Color fill;
  final Color color;
  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) => GestureDetector(
    onTap: onTap,
    behavior: HitTestBehavior.opaque,
    child: Container(
      width: 128,
      height: 30,
      alignment: Alignment.center,
      decoration: BoxDecoration(
        color: fill,
        borderRadius: BorderRadius.circular(15),
      ),
      child: Text(
        label,
        style: TextStyle(
          fontFamily: uiFontFamily,
          package: 'silica_ui',
          fontSize: 13,
          color: color,
        ),
      ),
    ),
  );
}

/// The 66 pt icon: a gray disc with the power glyph (or a person for log out).
class _PowerIconPainter extends CustomPainter {
  const _PowerIconPainter({
    required this.disc,
    required this.glyph,
    required this.person,
  });

  final Color disc;
  final Color glyph;
  final bool person;

  @override
  void paint(Canvas canvas, Size size) {
    final c = size.center(Offset.zero);
    canvas.drawCircle(c, size.width / 2, Paint()..color = disc);
    final stroke = Paint()
      ..color = glyph
      ..style = PaintingStyle.stroke
      ..strokeWidth = 4
      ..strokeCap = StrokeCap.round;
    if (person) {
      final fill = Paint()..color = glyph;
      canvas.drawCircle(c.translate(0, -8), 9, fill);
      canvas.drawArc(
        Rect.fromCenter(center: c.translate(0, 20), width: 36, height: 30),
        math.pi,
        math.pi,
        true,
        fill,
      );
      return;
    }
    // The power glyph: an open ring with a bar at the top.
    const r = 15.0;
    canvas.drawArc(
      Rect.fromCircle(center: c.translate(0, 2), radius: r),
      -math.pi / 2 + 0.6,
      2 * math.pi - 1.2,
      false,
      stroke,
    );
    canvas.drawLine(c.translate(0, -16), c.translate(0, -2), stroke);
  }

  @override
  bool shouldRepaint(_PowerIconPainter old) =>
      old.disc != disc || old.glyph != glyph || old.person != person;
}
