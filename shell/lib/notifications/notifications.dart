// Banner e Centro notifiche, da plan/reference/notifications.md (aspetto chiaro).
//
// Colonna larga 344 pt a 16 pt dal bordo destro e 16 pt sotto la barra dei menu. Carte di
// vetro "notifica" (σ ~10,5 pt, quasi opaco) con angoli di 20 pt: icona 32 pt a 14 pt dal
// bordo, testo da 58 pt, padding 12 sopra e 13 sotto, titolo semibold e corpo regolare a
// ~13,75 pt (maiuscole di 10 pt), stesso colore. Il banner entra da destra con la curva
// misurata; il Centro (comando notifications.toggle, clic sull'orologio) mostra le
// notifiche per app, con le carte impilate, le azioni e uno scrim ampio attorno alla
// colonna.
//
// Il componente resta in esecuzione su un layer overlay a schermo intero: senza banner né
// Centro non prende né puntatore né tastiera (silica/layer).

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

/// La superficie copre tutto lo schermo (zona esclusiva -1): la colonna parte sotto la
/// barra dei menu.
const double columnTop = barHeight + columnMargin;
const double cardRadius = 20;
const double iconSize = 32;
const double iconLeft = 14;
const double textLeft = 58;
const double paddingRight = 14;

/// Padding misurato: 12 sopra la riga del titolo, 13 sotto l'ultima. Inter a parità di
/// riga mette la maiuscola 2 pt più in alto: con 14 la maiuscola del titolo è a 15 pt dal
/// bordo come nel riferimento (y 61 su 46).
const double paddingTop = 14;
const double paddingBottom = 13;

/// Corpo del testo: le maiuscole misurano 10 pt, Inter le ha a 0,727 em.
const double fontSize = 13.75;
const double lineHeight = 16.5;
const double smallSize = 11;

/// Titolo, sottotitolo e corpo hanno lo stesso colore (`#1d1d1e` nei pixel più scuri).
const Color textColor = Color(0xFF1D1D1E);

/// Ora e testi secondari (`#575759`).
const Color secondaryColor = Color(0xFF575759);

/// Intestazione del Centro: bianco quasi pieno sopra lo scrim (`#e1e1e1` su `#5e5f63`).
const Color headerColor = Color(0xE0FFFFFF);

/// Intestazione del Centro: 21 pt, 15,5 pt sotto la carta sopra, 7,5 pt sopra la prima.
const double headerHeight = 21;
const double headerSize = 18.5;
const double headerGapBelow = 7.5;

/// Spazio fra le carte nel Centro (non misurato: come il passo delle carte impilate).
const double cardGap = 8;

/// Gruppo espanso (sessione 8, N-03): carte a passo 64 (6 pt fra l'una e l'altra),
/// intestazione con il titolo a 19 pt e i controlli da 20 pt; lo scambio di forma fra i
/// due controlli e l'espansione durano ~150–200 ms.
const double expandedGap = 6;
const double groupHeaderHeight = 28;
const double groupTitleSize = 19;
const double groupPillHeight = 20;
const double groupCollapseWidth = 102;
const double groupClearWidth = 65;
const Duration groupMorph = Duration(milliseconds: 180);

/// Carte impilate sotto un gruppo chiuso: strisce di 7 pt a passo 8, rientrate di ~19 e
/// ~29 pt per lato.
const double stackStep = 8;
const double stackStrip = 7;
const List<double> stackInsets = [19, 29];

/// Sezione delle azioni: più scura di ~20 livelli, separata da una riga di 1 pt; pulsanti
/// a capsula da 22 pt, due per riga con 8 pt fra loro, 12 pt dai bordi.
const Color actionsFill = Color(0x1E000000);
const Color actionsDivider = Color(0x38000000);
const Color buttonFill = Color(0x1C000000);
const Color buttonText = Color(0xFF2F2F31);
const double buttonHeight = 22;
const double actionsInset = 12;

/// Pulsanti che compaiono al passaggio del puntatore sul banner (sessione 8, N-02).
const double closeSize = 21;
const Offset closeCenter = Offset(4, 6);
const Size detailsSize = Size(65, 21);
const double detailsInset = 9;

/// Spazio fra il testo accorciato e la capsula (non misurato).
const double detailsGap = 8;

/// Lo scrim del Centro scurisce lo sfondo di ~×0,75 fra gli elementi e sfuma in ~60 pt.
const Color scrimColor = Color(0x40000000);
const double scrimBlur = 44;

/// Ombra del banner isolato: debolissima (−4 livelli sotto il bordo, −2 ai lati), verso
/// il basso, ~15 pt. Solo fuori dalla carta: dentro coprirebbe il vetro.
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

/// `HH:MM` dell'ora di ricezione, come nel Centro del riferimento.
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

  /// Con Non disturbare attivo le notifiche vanno nel Centro senza banner.
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
  /// Apertura e chiusura del Centro: il valore è il tempo trascorso, in proporzione alla
  /// durata della fase in corso.
  late final AnimationController _center;
  final FocusNode _focus = FocusNode();
  StreamSubscription<ShellNotification>? _posted;
  StreamSubscription<String>? _commands;

  /// I banner sullo schermo: al più uno che entra o resta, gli altri escono.
  final List<_BannerEntry> _banners = [];
  bool _centerOpen = false;
  final Set<String> _expanded = {};

  /// La dimensione della superficie all'ultimo layout.
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
    // Chiusa altrove (dal Centro, dall'app, scaduta): il banner esce.
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
        // Sostituita dall'app (stesso id): il contenuto cambia sul posto.
        same.first.notification = n;
        return;
      }
      // Una notifica nuova sostituisce il banner corrente, che esce mentre lei entra.
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

  /// Il puntatore serve solo dove c'è qualcosa: il banner, o tutto lo schermo con il
  /// Centro aperto (un clic fuori lo chiude).
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
    // La colonna in alto a destra, con lo spazio per il pulsante di chiusura.
    final left = size.width - columnMargin - columnWidth - 8;
    LayerControl.inputRegion([
      Rect.fromLTWH(left, barHeight, size.width - left, 260),
    ]);
  }

  void _activate(ShellNotification n) {
    if (n.hasDefault) widget.source.invoke(n.id, 'default');
  }

  /// Spostamento a destra del Centro e opacità nella fase corrente; null se è chiuso.
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

  /// Stato vuoto: un testo bianco in semibold, centrato sulla colonna.
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

/// L'intestazione di un gruppo espanso: il nome dell'app a sinistra (19 pt), a destra la
/// capsula per ridurre il gruppo (102 × 20) e il cerchio per cancellarlo (20 × 20). Con il
/// puntatore sul cerchio le due forme si scambiano la larghezza: il cerchio diventa la
/// capsula "Cancella" (65 × 20) e la capsula un cerchio con la freccia in su.
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
      decoration: const BoxDecoration(
        color: Color(0xFFB4B5B9),
        borderRadius: BorderRadius.all(Radius.circular(groupPillHeight / 2)),
      ),
      child: Text(
        label,
        maxLines: 1,
        overflow: TextOverflow.clip,
        style: _style(
          smallSize,
          const Color(0xFF3A3B3E),
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

/// Una carta del Centro: con l'ora, le azioni e il pulsante di chiusura al passaggio.
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

/// La carta di una notifica: vetro, icona, titolo, corpo; nel Centro anche ora e azioni.
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

  /// La capsula "Dettagli" a destra (banner al passaggio del puntatore): il testo si
  /// accorcia per lasciarle spazio.
  final bool showDetails;
  final VoidCallback? onDetails;

  @override
  Widget build(BuildContext context) {
    final n = notification;
    final buttons = showActions ? n.buttons : const <(String, String)>[];
    final card = Stack(
      clipBehavior: Clip.none,
      children: [
        if (shadow)
          const Positioned.fill(child: CustomPaint(painter: _OutsideShadow())),
        // La riga scura del bordo sta 1 pt fuori dalla cornice: il vetro è più largo di
        // 1 pt per lato e Ossido ci disegna la riga.
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
            children: [_content(n), if (buttons.isNotEmpty) _actions(buttons)],
          ),
        ),
      ],
    );
    return Stack(
      clipBehavior: Clip.none,
      children: [
        card,
        // La × al passaggio del puntatore: cerchio di 21 pt centrato 4 pt a destra e 6 pt
        // sotto l'angolo in alto a sinistra della carta.
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
                  decoration: const BoxDecoration(
                    color: buttonFill,
                    borderRadius: BorderRadius.all(Radius.circular(10.5)),
                  ),
                  child: Text('Dettagli', style: _style(smallSize, buttonText)),
                ),
              ),
            ),
          ),
      ],
    );
  }

  Widget _content(ShellNotification n) {
    final title = _style(fontSize, textColor, weight: FontWeight.w600);
    final body = _style(fontSize, textColor);
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
                        style: _style(smallSize, secondaryColor),
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

  Widget _actions(List<(String, String)> buttons) {
    return DecoratedBox(
      decoration: const BoxDecoration(
        color: actionsFill,
        border: Border(top: BorderSide(color: actionsDivider)),
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
                      decoration: const BoxDecoration(
                        color: buttonFill,
                        borderRadius: BorderRadius.all(
                          Radius.circular(buttonHeight / 2),
                        ),
                      ),
                      child: Text(
                        label,
                        maxLines: 1,
                        overflow: TextOverflow.ellipsis,
                        style: _style(fontSize, buttonText),
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

/// L'icona dell'app: l'immagine della notifica, l'icona chiesta, quella della voce
/// desktop, altrimenti una tessera con l'iniziale.
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
    // Angoli di ~7 pt su 32, contorno di 1 px più scuro (notifications.md).
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

/// Il cerchio con la × (chiudi, elimina tutto): fondo `#b4b5b9`, bordo scuro di 1 pt,
/// glifo di 2 px `#5a5b5c`.
class CloseCircle extends StatelessWidget {
  const CloseCircle({super.key, required this.size});

  final double size;

  @override
  Widget build(BuildContext context) =>
      CustomPaint(size: Size.square(size), painter: const _ClosePainter());
}

class _ClosePainter extends CustomPainter {
  const _ClosePainter();

  @override
  void paint(Canvas canvas, Size size) {
    final center = size.center(Offset.zero);
    final radius = size.width / 2;
    canvas.drawCircle(center, radius, Paint()..color = const Color(0xFF3A3B3E));
    canvas.drawCircle(
      center,
      radius - 1,
      Paint()..color = const Color(0xFFB4B5B9),
    );
    final arm = radius * 0.32;
    final stroke = Paint()
      ..color = const Color(0xFF5A5B5C)
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
  bool shouldRepaint(_ClosePainter old) => false;
}

/// Una carta che sporge sotto un gruppo chiuso.
class _StackStrip extends StatelessWidget {
  const _StackStrip();

  @override
  Widget build(BuildContext context) => ClipRRect(
    borderRadius: const BorderRadius.vertical(bottom: Radius.circular(10)),
    child: Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Expanded(child: Container(color: const Color(0xE6ADAEB2))),
        // Una riga più chiara in fondo (#bc–#be contro #a7–#af).
        Container(height: 1, color: const Color(0xFFBDBEC2)),
      ],
    ),
  );
}

/// Lo scrim del Centro: un alone scuro e morbido attorno a tutta la colonna (non per
/// carta), che sfuma in ~60 pt.
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
