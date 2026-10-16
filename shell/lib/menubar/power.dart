// The power confirmations of the logo menu (SH-MB-05, docs/spec/misc.md): Restart…, Shut
// Down… and Log Out… open an alert card with a 60-second countdown; at zero, or with the
// default button, the action runs. With ⌥ the menu items act at once, without the card.
//
// Card: Amalfi's system alert (Alerts/Alert/Two Buttons, glass variant: 280 wide, radius 20,
// Glass/Thick), centred in the upper third of the screen, above everything: a 56 pt gray icon
// at the top, the title and the body with the countdown, the "reopen windows" checkbox and
// two buttons (Cancel, and the default one). It zooms in with a small overshoot (~106 % at
// ~90 ms, 100 % by ~210 ms, measured: kept) while the content fades in; cancelling removes it
// in one frame. The texts are our own.
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

/// The card: the alert's width; the height depends on the title's lines.
const Size powerCardSize = Size(AmalfiAlertMetrics.width, 266);
const int powerCountdown = 60;

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
    final colors = AmalfiTheme.of(context).colors;
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
        child: AmalfiAlert(
          key: const ValueKey('power confirmation'),
          material: AmalfiAlertMaterial.glass,
          icon: CustomPaint(
            painter: _PowerIconPainter(
              disc: colors.surfaceSunken,
              glyph: colors.textSecondary,
              person: widget.action == PowerAction.logOut,
            ),
          ),
          title: title,
          message: body,
          accessory: GestureDetector(
            behavior: HitTestBehavior.opaque,
            onTap: () => setState(() => _reopen = !_reopen),
            child: Row(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                AmalfiCheckbox(
                  value: _reopen,
                  onChanged: (v) => setState(() => _reopen = v),
                ),
                const SizedBox(width: 8),
                // The label wraps: the alert is 280 wide.
                Expanded(
                  child: Text(
                    l.powerReopenWindows,
                    style: AmalfiType.body(color: colors.textPrimary),
                  ),
                ),
              ],
            ),
          ),
          actions: [
            AmalfiPushButton(
              key: const ValueKey('power cancel'),
              label: l.cancel,
              onPressed: widget.onCancel,
            ),
            AmalfiPushButton(
              key: const ValueKey('power confirm'),
              label: button,
              variant: AmalfiPushButtonVariant.primary,
              onPressed: widget.onConfirm,
            ),
          ],
        ),
      ),
    );
  }
}

/// The 56 pt icon: a gray disc with the power glyph (or a person for log out).
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
    // Drawn on the 66 pt grid it was measured on, scaled to the alert's 56 pt icon.
    canvas.save();
    canvas.scale(size.width / 66);
    size = const Size(66, 66);
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
      canvas.restore();
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
    canvas.restore();
  }

  @override
  bool shouldRepaint(_PowerIconPainter old) =>
      old.disc != disc || old.glyph != glyph || old.person != person;
}
