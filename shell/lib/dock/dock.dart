// Il Dock della shell: disposizione e ingrandimento da model.dart (misure in
// plan/reference/dock.md), fondo di Liquid Glass calcolato da Ossido, icone, separatore,
// indicatori ed etichetta disegnati qui.

import 'dart:async';
import 'dart:math' as math;
import 'dart:io' show FileSystemEntity, Platform;
import 'dart:ui' as ui;

import 'package:flutter/scheduler.dart';
import 'package:flutter/services.dart' show KeyDownEvent, LogicalKeyboardKey;
import 'package:flutter/widgets.dart';
import 'package:silica_ui/silica_ui.dart';

import 'package:flutter/gestures.dart' show kSecondaryButton;

import '../system/desktop_entries.dart';
import '../system/layer_control.dart';
import '../system/shell_channel.dart';
import '../system/shell_icon.dart';
import '../system/silica_icons.dart';
import 'apps.dart';
import 'icons.dart';
import 'stack.dart';
import 'trash_alert.dart';
import 'model.dart';

/// Il Dock con le app vere: installate (.desktop), fissate e aperte (da Ossido).
class LiveDock extends StatefulWidget {
  const LiveDock({super.key});

  @override
  State<LiveDock> createState() => _LiveDockState();
}

class _LiveDockState extends State<LiveDock> {
  late final Map<String, DesktopEntry> _entries = loadDesktopEntries();
  late final List<String> _pinned = pinnedIds(_entries);

  /// Le finestre dalla più recente (focus): un clic sull'app porta in cima l'ultima usata.
  final List<String> _recent = [];

  /// L'avviso "Svuota il Cestino" aperto.
  bool _confirmEmpty = false;

  /// Le miniature delle finestre ridotte, chieste a Ossido alla riduzione.
  final Map<String, ui.Image> _thumbnails = {};

  @override
  void initState() {
    super.initState();
    ShellChannel.instance.toplevels.addListener(_changed);
    ShellChannel.instance.focused.addListener(_focused);
    ShellChannel.instance.minimized.addListener(_minimized);
    ShellChannel.instance.refresh();
  }

  void _minimized() {
    final ids = ShellChannel.instance.minimized.value;
    _thumbnails.removeWhere((id, image) {
      final gone = !ids.contains(id);
      if (gone) image.dispose();
      return gone;
    });
    for (final id in ids) {
      if (_thumbnails.containsKey(id)) continue;
      ShellChannel.instance.thumbnail(id).then((image) {
        if (!mounted || image == null) return;
        if (ShellChannel.instance.minimized.value.contains(id)) {
          setState(() => _thumbnails[id] = image);
        } else {
          image.dispose();
        }
      });
    }
    setState(() {});
  }

  @override
  void dispose() {
    ShellChannel.instance.toplevels.removeListener(_changed);
    ShellChannel.instance.focused.removeListener(_focused);
    ShellChannel.instance.minimized.removeListener(_minimized);
    for (final image in _thumbnails.values) {
      image.dispose();
    }
    super.dispose();
  }

  void _changed() => setState(() {});

  void _focused() {
    final id = ShellChannel.instance.focused.value;
    if (id.isEmpty) return;
    _recent
      ..remove(id)
      ..insert(0, id);
  }

  /// La finestra dell'app usata per ultima.
  String _latest(Item item) {
    for (final id in _recent) {
      if (item.windows.contains(id)) return id;
    }
    return item.windows.first;
  }

  void _activate(Item item) {
    // Una finestra ridotta torna lei; un'app con tutte le finestre ridotte riporta l'ultima
    // ridotta (D-03); un'app aperta torna in primo piano; altrimenti si avvia.
    final minimized = ShellChannel.instance.minimized.value;
    if (item.minimizedWindow case final window?) {
      ShellChannel.instance.activate(window);
    } else if (item.windows.isNotEmpty &&
        item.windows.every(minimized.contains)) {
      ShellChannel.instance.activate(
        minimized.lastWhere(item.windows.contains),
      );
    } else if (item.windows.isNotEmpty) {
      ShellChannel.instance.activate(_latest(item));
    } else if (item.command case final command?) {
      ShellChannel.instance.launch(command);
    }
  }

  void _setPinned(Item item, bool pinned) {
    final id = item.id;
    if (id == null) return;
    setState(() {
      _pinned.remove(id);
      if (pinned) _pinned.add(id);
    });
    savePinned(_pinned);
  }

  /// Il menu di un'icona (dock.md, D-04): le finestre aperte, Opzioni, poi le azioni.
  List<MenuEntry> _menu(Item item) {
    final shell = ShellChannel.instance;
    final toplevels = shell.toplevels.value;
    if (item.minimizedWindow case final window?) {
      return [
        MenuAction(
          'Apri "${item.label}"',
          onSelected: () => shell.activate(window),
        ),
      ];
    }
    // Cartelle e cestino: Apri; il Cestino pieno anche Svuota (D-07).
    if (item.id == null) {
      return [
        if (item.command case final command?)
          MenuAction('Apri', onSelected: () => shell.launch(command)),
        if (item.label == 'Cestino' &&
            trashHasFiles(Platform.environment['HOME'] ?? '')) ...[
          const MenuDivider(),
          MenuAction(
            'Svuota il Cestino…',
            onSelected: () => setState(() => _confirmEmpty = true),
          ),
        ],
      ];
    }
    return [
      for (final id in item.windows)
        MenuAction(
          toplevels.where((t) => t.id == id).firstOrNull?.title ?? item.label,
          onSelected: () => shell.activate(id),
        ),
      if (item.windows.isNotEmpty) const MenuDivider(),
      MenuAction(
        'Opzioni',
        submenu: [
          if (item.pinned)
            MenuAction(
              'Rimuovi dal Dock',
              onSelected: () => _setPinned(item, false),
            )
          else
            MenuAction(
              'Mantieni nel Dock',
              onSelected: () => _setPinned(item, true),
            ),
        ],
      ),
      const MenuDivider(),
      if (item.running) ...[
        MenuAction(
          'Mostra tutte le finestre',
          onSelected: () {
            shell.activate(_latest(item));
            shell.execute('panorama.app');
          },
        ),
        MenuAction(
          'Esci',
          onSelected: () {
            shell.activate(_latest(item));
            shell.execute('app.quit');
          },
        ),
      ] else
        MenuAction('Apri', onSelected: () => _activate(item)),
    ];
  }

  @override
  Widget build(BuildContext context) => Dock(
    items: dockItems(
      _entries,
      _pinned,
      ShellChannel.instance.toplevels.value,
      minimized: ShellChannel.instance.minimized.value,
      thumbnails: _thumbnails,
    ),
    onActivate: _activate,
    menuFor: _menu,
    onOpenPath: (path) =>
        ShellChannel.instance.launch('xdg-open ${shellQuote(path)}'),
    alert: _confirmEmpty
        ? TrashAlert(
            onEmpty: () async {
              setState(() => _confirmEmpty = false);
              await emptyTrash();
              if (mounted) setState(() {});
            },
            onCancel: () => setState(() => _confirmEmpty = false),
          )
        : null,
    onReorder: (order, dropped) {
      // Le fissate nel nuovo ordine; un'app aperta lasciata cadere fra loro diventa fissata.
      final ids = [
        for (final item in order)
          if (item.id case final id?
              when item.pinned || identical(item, dropped))
            id,
      ];
      setState(() {
        _pinned
          ..clear()
          ..addAll(ids);
      });
      savePinned(_pinned);
    },
  );
}

/// Colori dell'aspetto chiaro (dock.md).
const Color separatorColor = Color(0xFF484950);
const Color indicatorColor = Color(0xFF03040B);
const Color labelBody = Color(0xFFBDBEC3);

/// Menu dell'icona (D-04): compare dopo ~575 ms di pressione (550–600 misurati), si trascina
/// oltre 5 pt; la punta tocca ~11 pt sopra il fondo del Dock.
const Duration longPressMenu = Duration(milliseconds: 575);
const double dragThreshold = 5;
const double menuTipAboveDock = 11;

/// Spazio sopra il menu per la sua ombra quando la superficie si allunga.
const double menuShadowRoom = 30;

/// La regione d'input del Dock: il fondo, più quanto serve all'ingrandimento e all'isteresi.
const double dockInputMargin = 60;
const double dockInputAbove = 60;

/// Trascinamento (D-05): il posto vuoto, lo scorrimento delle vicine (~265 ms, a S),
/// "Rimuovi" a ~130 pt sopra la presa, dissolvenza dell'elemento tolto (~250 ms). Il
/// puntatore conta come "nel Dock" fino a 20 pt sopra il fondo.
const Duration gapSlide = Duration(milliseconds: 265);
const double removeAbove = 130;
const Duration removeFade = Duration(milliseconds: 250);
const double dragInsideAbove = 20;

/// Il posto vuoto dove cadrebbe l'icona trascinata.
const Item gapItem = Item.app('');

/// Un trascinamento: l'elemento preso, il punto di presa nell'icona, dove è il puntatore.
class _DockDrag {
  _DockDrag({
    required this.index,
    required this.item,
    required this.grab,
    required this.side,
    required this.downY,
    required this.pointer,
  });

  final int index;
  final Item item;
  final Offset grab;
  final double side;
  final double downY;
  Offset pointer;
  bool started = false;
  bool inside = true;
  bool remove = false;
}

/// L'elemento tolto che sfuma: icona, posizione, lato e inizio.
class _Ghost {
  _Ghost(this.item, this.at, this.side);
  final Item item;
  final Offset at;
  final double side;
  Duration elapsed = Duration.zero;
}

/// Un argomento per `sh -c`, fra apici singoli.
String shellQuote(String text) => "'${text.replaceAll("'", r"'\''")}'";

/// Finestre ridotte nel Dock (D-08): miniatura 57 × 47 nella cella da 63, icona dell'app da
/// 18 pt che sporge 4 pt sotto la miniatura.
const double minimizedThumbWidth = 57;
const double minimizedThumbHeight = 47;
const double minimizedBadge = 18;
const double minimizedBadgeBelow = 4;

/// Icona premuta: scurita (stima, il valore non è misurato).
const Color pressedShade = Color(0x40000000);
const Color labelBodyTop = Color(0xFFD7D8DE);
const Color labelBodyBottom = Color(0xFFC1C2C8);
const Color labelBorder = Color(0xFF3C3D41);
const Color labelText = Color(0xFF1C1C1D);

/// Etichetta: corpo alto 27 pt (y 1325–1351), punta larga ~7 pt e alta ~3 pt, con la cima
/// ~13 pt sopra l'icona (icona da 1367, punta a 1354), margine orizzontale ~15,5 pt, testo
/// di ~14 pt (dalle maiuscole di 10 pt in Inter).
const double labelHeight = 27;
const double labelTip = 3;
const double labelTipHalfWidth = 3.5;
const double labelGap = 13;
const double labelPadding = 15.5;
const double labelFontSize = 14;

class Dock extends StatefulWidget {
  const Dock({
    super.key,
    required this.items,
    this.magnification = defaultMagnification,
    this.onActivate,
    this.menuFor,
    this.onReorder,
    this.alert,
    this.onOpenPath,
  });

  final List<Item> items;
  final double magnification;

  /// Clic su un'app.
  final void Function(Item item)? onActivate;

  /// Un avviso modale sopra lo schermo (Svuota il Cestino), grande [trashAlertSize].
  final Widget? alert;

  /// Un file della pila aperta, o la cartella intera ("Apri la cartella").
  final ValueChanged<String>? onOpenPath;

  /// Il menu di un elemento (clic destro, pressione lunga); null o vuoto: niente menu.
  final List<MenuEntry> Function(Item item)? menuFor;

  /// Trascinamento (D-05): il nuovo ordine delle app a sinistra del separatore e l'elemento
  /// lasciato cadere (null se è stato tolto con "Rimuovi", e allora manca dall'ordine).
  /// Null: le icone non si trascinano.
  final void Function(List<Item> order, Item? dropped)? onReorder;

  @override
  State<Dock> createState() => _DockState();
}

class _DockState extends State<Dock> with TickerProviderStateMixin {
  late final Ticker _ticker = createTicker(_tick);

  /// L'etichetta: l'elemento a cui appartiene e la sua opacità.
  int? _labelIndex;
  late final AnimationController _labelAnim = AnimationController(vsync: this)
    ..addListener(() => setState(() {}));
  Timer? _labelTimer;

  /// Il menu aperto: l'elemento, le voci, se è nato da una pressione lunga.
  int? _menuIndex;
  List<MenuEntry> _menuEntries = const [];
  bool _menuByLongPress = false;
  final GlobalKey<MenuHostState> _menuKey = GlobalKey();
  Timer? _longPress;
  Offset? _downAt;
  int? _downIndex;

  /// Il trascinamento in corso e l'icona tolta che sfuma sul posto.
  _DockDrag? _drag;
  _Ghost? _ghost;
  DateTime? _settleUntil;
  late final Ticker _ghostTicker = createTicker((elapsed) {
    final ghost = _ghost;
    if (ghost == null || elapsed >= removeFade) {
      _ghostTicker.stop();
      setState(() => _ghost = null);
    } else {
      setState(() => ghost.elapsed = elapsed);
    }
  });

  /// La pila aperta (D-06): l'elemento, i file, la vista e l'animazione (0 chiusa, 1 aperta).
  int? _stackIndex;
  List<FileSystemEntity> _stackEntries = const [];
  StackView _stackView = StackView.fan;
  bool _stackClosing = false;
  late final AnimationController _stackAnim = AnimationController(
    vsync: this,
    duration: stackOpen,
  )..addListener(() => setState(() {}));
  final FocusNode _stackFocus = FocusNode();

  /// L'altezza di partenza della superficie e lo schermo (per centrare l'avviso).
  double? _baseHeight;
  Size? _screen;

  /// Dove sta la pila aperta nella superficie, per la regione d'input e i clic.
  Rect? _stackRect;

  /// Quello che si è chiesto al layer per ultimo (regione d'input, altezza, tastiera).
  String _layerState = '';

  /// L'app premuta (icona scurita) e quelle che rimbalzano, con l'istante d'inizio.
  int? _pressed;
  final Map<String, DateTime> _bounces = {};
  late final Ticker _bounceTicker = createTicker((_) {
    _endBounces();
    setState(() {});
  });
  double? _pointerX;
  double _amount = 0;
  bool _inside = false;
  // Animazione dell'entrata o dell'uscita: da dove parte e quando.
  double _from = 0;
  Duration? _start;

  @override
  void dispose() {
    _ticker.dispose();
    _bounceTicker.dispose();
    _labelAnim.dispose();
    _labelTimer?.cancel();
    _longPress?.cancel();
    _ghostTicker.dispose();
    _stackAnim.dispose();
    _stackFocus.dispose();
    super.dispose();
  }

  /// L'ultimo elemento a sinistra del separatore (dove si può trascinare).
  int get _leftEnd {
    final separator = widget.items.indexWhere(
      (i) => i.kind == ItemKind.separator,
    );
    return separator < 0 ? widget.items.length : separator;
  }

  /// Gli elementi da disporre: durante il trascinamento senza quello preso e, se il puntatore
  /// è nel Dock, con un posto vuoto dove cadrebbe.
  List<Item> _displayItems(Size size) {
    final drag = _drag;
    if (drag == null || !drag.started) return widget.items;
    final rest = [...widget.items]..removeAt(drag.index);
    if (!drag.inside || drag.remove) return rest;
    final slot = _dropSlot(rest, size, drag.pointer.dx);
    return [...rest]..insert(slot, gapItem);
  }

  /// Dove cadrebbe l'icona: il primo elemento a sinistra il cui centro è oltre il puntatore.
  int _dropSlot(List<Item> rest, Size size, double x) {
    final l = layout(rest, width: size.width, height: size.height);
    final end = rest.indexWhere((i) => i.kind == ItemKind.separator);
    final limit = end < 0 ? rest.length : end;
    for (var i = 0; i < limit; i++) {
      if (x < l.center(i)) return i;
    }
    return limit;
  }

  void _startDrag(Offset p, Size size) {
    final index = _downIndex;
    if (index == null || index >= _leftEnd || widget.onReorder == null) return;
    final item = widget.items[index];
    if (item.kind != ItemKind.app) return;
    final l = layout(
      widget.items,
      width: size.width,
      height: size.height,
      pointerX: _pointerX,
      amount: _amount,
      magnification: widget.magnification,
    );
    final (x, y, side) = iconRect(l, index, size.height);
    _longPress?.cancel();
    _longPress = null;
    _showLabel(null);
    setState(() {
      _pressed = null;
      _drag = _DockDrag(
        index: index,
        item: item,
        grab: p - Offset(x, y),
        side: side,
        downY: _downAt?.dy ?? p.dy,
        pointer: p,
      )..started = true;
    });
  }

  void _moveDrag(Offset p, Size size) {
    final drag = _drag;
    if (drag == null) return;
    final rest = layout(widget.items, width: size.width, height: size.height);
    setState(() {
      drag.pointer = p;
      drag.inside =
          inActiveZone(rest, p.dx, p.dy, size.height) ||
          p.dy >= size.height - bottomGap - backgroundHeight - dragInsideAbove;
      // "Rimuovi" compare 125–135 pt sopra il punto di presa.
      drag.remove = p.dy < drag.downY - removeAbove;
    });
  }

  void _endDrag(Size size) {
    final drag = _drag;
    if (drag == null) return;
    final rest = [...widget.items]..removeAt(drag.index);
    final left = rest.indexWhere((i) => i.kind == ItemKind.separator);
    final leftItems = rest.sublist(0, left < 0 ? rest.length : left);
    if (drag.remove) {
      // Tolta: l'icona e la capsula sfumano sul posto (~250 ms), senza altri effetti.
      _ghost = _Ghost(drag.item, drag.pointer - drag.grab, drag.side);
      _ghostTicker.start();
      widget.onReorder?.call(leftItems, null);
    } else if (drag.inside) {
      final slot = _dropSlot(rest, size, drag.pointer.dx);
      final order = [...leftItems]
        ..insert(slot.clamp(0, leftItems.length), drag.item);
      widget.onReorder?.call(order, drag.item);
    }
    setState(() {
      _drag = null;
      _settleUntil = DateTime.now().add(gapSlide);
    });
  }

  /// Apre il menu di un elemento (D-04): l'icona resta premuta, l'etichetta sparisce.
  void _openMenu(int index, {required bool longPress}) {
    final entries = widget.menuFor?.call(widget.items[index]) ?? const [];
    if (entries.isEmpty) return;
    setState(() {
      _menuIndex = index;
      _menuEntries = entries;
      _menuByLongPress = longPress;
      _pressed = index;
    });
    _showLabel(null);
  }

  void _closeMenu() {
    if (!mounted) return;
    setState(() {
      _menuIndex = null;
      _menuEntries = const [];
      _pressed = null;
    });
  }

  bool _menuContains(Offset p) => _menuKey.currentState?.contains(p) ?? false;

  void _pointerDown(PointerDownEvent e, Layout l, Size size) {
    final p = e.localPosition;
    // Con la pila aperta un clic fuori da lei e dalla sua icona la chiude.
    if (_stackIndex != null &&
        !(_stackRect?.contains(p) ?? false) &&
        itemAt(l, widget.items, p.dx) != _stackIndex) {
      _closeStack();
      return;
    }
    if (_menuIndex != null) {
      if (_menuContains(p)) {
        _menuKey.currentState?.pointerDown(p);
      } else {
        _menuKey.currentState?.close();
      }
      return;
    }
    final rest = layout(widget.items, width: size.width, height: size.height);
    final index = inActiveZone(rest, p.dx, p.dy, size.height) || _inside
        ? itemAt(l, widget.items, p.dx)
        : null;
    if (index == null) return;
    if (e.buttons & kSecondaryButton != 0) {
      _openMenu(index, longPress: false);
      return;
    }
    setState(() => _pressed = index);
    _downAt = p;
    _downIndex = index;
    _longPress?.cancel();
    _longPress = Timer(longPressMenu, () {
      _longPress = null;
      _openMenu(index, longPress: true);
    });
  }

  void _pointerMove(Offset p, Size size) {
    if (_drag != null) {
      _moveDrag(p, size);
      return;
    }
    if (_longPress != null &&
        _downAt != null &&
        (p - _downAt!).distance > dragThreshold) {
      // Oltre 5 pt la pressione diventa trascinamento.
      _startDrag(p, size);
      _longPress?.cancel();
      _longPress = null;
    }
    if (_menuIndex != null) _menuKey.currentState?.pointerMoved(p);
  }

  void _pointerUp(PointerUpEvent e, Layout l, Size size) {
    final p = e.localPosition;
    if (_drag != null) {
      _endDrag(size);
      return;
    }
    if (_longPress != null) {
      // Un clic: l'elemento sotto il puntatore, se è lo stesso della pressione.
      _longPress?.cancel();
      _longPress = null;
      final index = itemAt(l, widget.items, p.dx);
      setState(() => _pressed = null);
      if (index != null && index == _downIndex) _activate(index);
      return;
    }
    if (_menuIndex != null && _menuByLongPress) {
      // Dopo la pressione lunga: rilascio su una voce la sceglie, sull'icona chiude.
      _menuByLongPress = false;
      if (_menuContains(p)) {
        _menuKey.currentState?.pointerUp(p);
      } else {
        _menuKey.currentState?.close();
      }
    }
  }

  /// Regione d'input, altezza della superficie e tastiera: solo quando cambiano.
  void _syncLayer(Rect dock, Rect? menu, Rect? alert, Size size) {
    // La pila aperta occupa la superficie alta quanto lo schermo, come l'avviso.
    alert ??= _stackIndex != null ? _stackRect ?? Rect.zero : null;
    _baseHeight ??= size.height;
    final base = _baseHeight ?? size.height;
    // Con un menu la superficie arriva fin sopra il menu, con l'avviso a tutto lo schermo;
    // senza, torna all'altezza di partenza.
    final height = alert != null
        ? _screen?.height ?? size.height
        : menu != null
        ? (size.height - menu.top + menuShadowRoom).clamp(size.height, 4096.0)
        : base;
    final key = '$dock $menu $alert $height';
    if (key == _layerState) return;
    _layerState = key;
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (height != size.height) LayerControl.size(0, height);
      LayerControl.inputRegion([
        dock,
        if (menu != null) menu.inflate(4),
        if (alert != null) alert,
      ]);
      LayerControl.keyboard(
        menu == null && alert == null ? 'none' : 'exclusive',
      );
    });
  }

  static String _key(Item item) => '${item.label}\u0000${item.command}';

  /// Un salto in corso finisce a terra quando l'app ha una finestra, o dopo il limite.
  void _endBounces() {
    final now = DateTime.now();
    _bounces.removeWhere((key, start) {
      final ms = now.difference(start).inMicroseconds / 1000;
      final item = widget.items.where((i) => _key(i) == key).firstOrNull;
      final landed = ms % bouncePeriod < 30 && ms > bouncePeriod / 2;
      return ms > bounceLimit.inMilliseconds ||
          item == null ||
          (item.running && landed);
    });
    if (_bounces.isEmpty && _bounceTicker.isActive) _bounceTicker.stop();
  }

  double _bounce(int index) => _bounceOf(widget.items[index]);

  double _bounceOf(Item item) {
    final start = _bounces[_key(item)];
    if (start == null) return 0;
    return bounceHeight(DateTime.now().difference(start).inMicroseconds / 1000);
  }

  void _openStack(int index) {
    final folder = widget.items[index].folder;
    if (folder == null) return;
    final entries = stackEntries(folder);
    setState(() {
      _stackIndex = index;
      _stackEntries = entries;
      _stackView = automaticView(entries.length);
      _stackClosing = false;
    });
    _showLabel(null);
    _stackAnim.duration = stackOpen;
    _stackAnim.forward(from: 0);
    WidgetsBinding.instance.addPostFrameCallback(
      (_) => _stackFocus.requestFocus(),
    );
  }

  Future<void> _closeStack() async {
    if (_stackIndex == null || _stackClosing) return;
    _stackClosing = true;
    _stackAnim.duration = _stackView == StackView.fan ? fanClose : gridClose;
    await _stackAnim.reverse();
    if (mounted) {
      setState(() {
        _stackIndex = null;
        _stackClosing = false;
      });
    }
  }

  /// Un file della pila: la pila si chiude subito, poi il file si apre.
  void _openFromStack(String path) {
    _closeStack();
    widget.onOpenPath?.call(path);
  }

  void _activate(int index) {
    final item = widget.items[index];
    if (item.folder != null && widget.onOpenPath != null) {
      _stackIndex == index ? _closeStack() : _openStack(index);
      return;
    }
    if (!item.running && item.command != null) {
      _bounces.putIfAbsent(_key(item), DateTime.now);
      if (!_bounceTicker.isActive) _bounceTicker.start();
    }
    widget.onActivate?.call(item);
  }

  /// L'etichetta segue la cella sotto il puntatore: subito da un'icona all'altra.
  void _showLabel(int? index) {
    if (index != null) {
      _labelTimer?.cancel();
      _labelTimer = null;
      final hidden = _labelIndex == null || _labelAnim.value < 1;
      _labelIndex = index;
      if (hidden) {
        _labelAnim.animateTo(1, duration: labelAppear);
      }
      return;
    }
    if (_labelIndex == null || _labelTimer != null) return;
    _labelTimer = Timer(labelHold, () {
      _labelTimer = null;
      _labelAnim.animateTo(0, duration: labelVanish).whenComplete(() {
        if (_labelAnim.value == 0 && mounted)
          setState(() => _labelIndex = null);
      });
    });
  }

  void _tick(Duration elapsed) {
    _start ??= elapsed;
    final duration = enterDuration(widget.magnification);
    final t = (elapsed - _start!).inMicroseconds / duration.inMicroseconds;
    final target = _inside ? 1.0 : 0.0;
    final eased = enterProgress(t);
    setState(() => _amount = _from + (target - _from) * eased);
    if (t >= 1) {
      _ticker.stop();
      _start = null;
      if (!_inside) _pointerX = null;
    }
  }

  void _setInside(bool inside) {
    if (inside == _inside) return;
    _inside = inside;
    _from = _amount;
    _start = null;
    if (_ticker.isActive) _ticker.stop();
    _ticker.start();
  }

  void _hover(Offset position, Size size) {
    final rest = layout(widget.items, width: size.width, height: size.height);
    final current = layout(
      widget.items,
      width: size.width,
      height: size.height,
      pointerX: _pointerX,
      amount: _amount,
      magnification: widget.magnification,
    );
    // Si entra dalla soglia fissa; si esce solo lasciando la cella ingrandita (isteresi).
    final inside = _inside
        ? staysInActiveZone(
            rest,
            current,
            widget.items,
            position.dx,
            position.dy,
            size.height,
          )
        : inActiveZone(rest, position.dx, position.dy, size.height);
    setState(() {
      // L'ingrandimento segue il puntatore senza inerzia; uscendo resta l'ultima x.
      if (inside) _pointerX = position.dx;
    });
    _setInside(inside);
    _showLabel(inside ? itemAt(current, widget.items, position.dx) : null);
  }

  @override
  Widget build(BuildContext context) {
    return LayoutBuilder(
      builder: (context, constraints) {
        final size = constraints.biggest;
        final items = _displayItems(size);
        final dragging = _drag != null;
        final l = layout(
          items,
          width: size.width,
          height: size.height,
          pointerX: _pointerX,
          amount: _amount,
          magnification: widget.magnification,
        );
        final (bx, by, bw, bh) = l.background;
        final label = _labelIndex;
        final hovered =
            !dragging && label != null && label < widget.items.length
            ? label
            : null;
        // Durante e subito dopo il trascinamento le icone scorrono al loro posto (~265 ms).
        final settle = _settleUntil;
        final sliding =
            dragging || (settle != null && DateTime.now().isBefore(settle));
        if (sliding && !dragging) {
          WidgetsBinding.instance.scheduleFrame();
        }
        // La punta del menu: sopra l'icona, ~11 pt sopra il fondo del Dock (D-04).
        final menuIndex = _menuIndex;
        final tip = menuIndex == null || menuIndex >= l.cells.length
            ? null
            : Offset(l.center(menuIndex), by - menuTipAboveDock);
        final menuRect = tip == null
            ? null
            : () {
                final m = menuSize(_menuEntries);
                return Rect.fromLTWH(
                  tip.dx - MenuMetrics.tipBodyLeft,
                  tip.dy - MenuMetrics.tipHeight - m.height,
                  m.width,
                  m.height + MenuMetrics.tipHeight,
                );
              }();
        // Fuori dal Dock (e dal menu) il puntatore va alle finestre sotto.
        final dockRect = Rect.fromLTRB(
          bx - dockInputMargin,
          by - dockInputAbove,
          bx + bw + dockInputMargin,
          size.height,
        );
        // L'avviso (D-07): centrato in orizzontale, la cima al ~20 % dello schermo, quando
        // la superficie è alta quanto lo schermo.
        final screen = _screen;
        final alert = widget.alert;
        if (alert != null && screen == null) {
          LayerControl.outputSize().then((size) {
            if (mounted)
              setState(() => _screen = size ?? const Size(1280, 800));
          });
        }
        final alertRect = alert == null || screen == null
            ? null
            : Rect.fromLTWH(
                (size.width - trashAlertSize.width) / 2,
                screen.height * trashAlertTop - (screen.height - size.height),
                trashAlertSize.width,
                trashAlertSize.height,
              );
        // La pila: serve lo schermo intero sopra il Dock.
        final stackIndex = _stackIndex;
        if (stackIndex != null && screen == null) {
          LayerControl.outputSize().then((value) {
            if (mounted)
              setState(() => _screen = value ?? const Size(1280, 800));
          });
        }
        final zoneTop = by - activeZoneAbove;
        Widget? stackWidget;
        if (stackIndex != null && stackIndex < l.cells.length) {
          final iconX = l.center(stackIndex);
          final item = widget.items[stackIndex];
          final progress = Curves.easeOut.transform(_stackAnim.value);
          if (_stackView == StackView.fan) {
            final count = math.min(_stackEntries.length, fanMax) + 1;
            final top = fanSlot(count - 1);
            _stackRect = Rect.fromLTRB(
              iconX - 420,
              zoneTop + top.center.dy - top.side,
              iconX + fanShift.last + 120,
              zoneTop,
            );
            stackWidget = Positioned.fill(
              child: FanStack(
                entries: _stackEntries,
                anchor: Offset(iconX, zoneTop),
                progress: progress,
                onOpen: (e) => _openFromStack(e.path),
                onOpenFolder: () => _openFromStack(item.folder ?? ''),
              ),
            );
          } else {
            final panel = gridPanelSize(_stackEntries.length, zoneTop - 40);
            final double left = (iconX - panel.width / 2)
                .clamp(8.0, math.max(8.0, size.width - panel.width - 8))
                .toDouble();
            final rect = Rect.fromLTWH(
              left,
              by - gridTipAboveDock - gridTipHeight - panel.height,
              panel.width,
              panel.height,
            );
            _stackRect = rect;
            stackWidget = GridStack(
              title: item.label,
              entries: _stackEntries,
              rect: rect,
              tipX: iconX - left,
              progress: progress,
              onOpen: (e) => _openFromStack(e.path),
              onOpenFolder: () => _openFromStack(item.folder ?? ''),
            );
          }
        } else {
          _stackRect = null;
        }
        _syncLayer(dockRect, menuRect, alertRect, size);
        return Listener(
          behavior: HitTestBehavior.translucent,
          onPointerDown: (e) => _pointerDown(e, l, size),
          onPointerMove: (e) => _pointerMove(e.localPosition, size),
          onPointerHover: (e) {
            if (_menuIndex != null)
              _menuKey.currentState?.pointerMoved(e.localPosition);
          },
          onPointerUp: (e) => _pointerUp(e, l, size),
          onPointerCancel: (_) {
            if (_drag != null) setState(() => _drag = null);
            _longPress?.cancel();
            _longPress = null;
            if (_menuIndex == null) setState(() => _pressed = null);
          },
          child: MouseRegion(
            onHover: (event) => _hover(event.localPosition, size),
            onExit: (_) {
              _setInside(false);
              _showLabel(null);
            },
            child: Stack(
              children: [
                Positioned(
                  left: bx,
                  top: by,
                  width: bw,
                  height: bh,
                  child: const LiquidGlass(
                    radius: backgroundRadius,
                    material: GlassMaterial.clear,
                    interactive: false,
                    child: SizedBox.expand(),
                  ),
                ),
                Positioned.fill(
                  child: CustomPaint(
                    painter: _DockPainter(items, l, size.height),
                  ),
                ),
                for (var i = 0; i < items.length; i++)
                  if (items[i].minimizedWindow != null)
                    _windowCell(
                      l,
                      i,
                      iconRect(l, i, size.height).$1,
                      iconRect(l, i, size.height).$2,
                      iconRect(l, i, size.height).$3,
                      items[i].icon,
                      items[i],
                    )
                  else if (items[i].icon case final source?)
                    _icon(l, i, size.height, source, items[i], sliding),
                if (_drag case final drag?) ..._dragged(drag),
                if (_ghost case final ghost?) _faded(ghost),
                if (hovered != null && _menuIndex == null)
                  _label(
                    l,
                    hovered,
                    size.height,
                    _labelAnim.value.clamp(0.0, 1.0),
                  ),
                if (stackWidget != null &&
                    (_screen == null || size.height >= _screen!.height - 1))
                  Focus(
                    focusNode: _stackFocus,
                    onKeyEvent: (node, event) {
                      if (event is KeyDownEvent &&
                          event.logicalKey == LogicalKeyboardKey.escape) {
                        _closeStack();
                        return KeyEventResult.handled;
                      }
                      return KeyEventResult.ignored;
                    },
                    child: Stack(
                      clipBehavior: Clip.none,
                      children: [stackWidget],
                    ),
                  ),
                if (alertRect != null && alert != null && alertRect.top >= 0)
                  Positioned.fromRect(rect: alertRect, child: alert),
                if (tip != null)
                  Positioned.fill(
                    child: MenuHost(
                      key: _menuKey,
                      entries: _menuEntries,
                      anchor: Offset.zero,
                      tip: tip,
                      handlesPointer: false,
                      onDismissed: _closeMenu,
                    ),
                  ),
              ],
            ),
          ),
        );
      },
    );
  }

  /// L'icona trascinata: a grandezza piena, opaca, agganciata al punto di presa; sopra, la
  /// capsula "Rimuovi" quando è abbastanza in alto.
  List<Widget> _dragged(_DockDrag drag) {
    final source = drag.item.icon;
    final at = drag.pointer - drag.grab;
    return [
      if (source != null)
        Positioned(
          left: at.dx,
          top: at.dy,
          width: drag.side,
          height: drag.side,
          child: ShellIcon(source, side: drag.side),
        ),
      if (drag.remove)
        _capsule('Rimuovi', at.dx + drag.side / 2, at.dy - labelGap, 1),
    ];
  }

  /// L'elemento tolto sfuma sul posto con la sua capsula.
  Widget _faded(_Ghost ghost) {
    final t = ghost.elapsed.inMicroseconds / removeFade.inMicroseconds;
    final opacity = (1 - t).clamp(0.0, 1.0);
    final source = ghost.item.icon;
    return Positioned.fill(
      child: IgnorePointer(
        child: Opacity(
          opacity: opacity,
          child: Stack(
            children: [
              if (source != null)
                Positioned(
                  left: ghost.at.dx,
                  top: ghost.at.dy,
                  width: ghost.side,
                  height: ghost.side,
                  child: ShellIcon(source, side: ghost.side),
                ),
              _capsule(
                'Rimuovi',
                ghost.at.dx + ghost.side / 2,
                ghost.at.dy - labelGap,
                1,
              ),
            ],
          ),
        ),
      ),
    );
  }

  /// Una capsula come l'etichetta, con la punta in (`centerX`, `tipY`).
  Widget _capsule(String text, double centerX, double tipY, double opacity) {
    final style = uiText(size: labelFontSize, color: labelText);
    final painter = TextPainter(
      text: TextSpan(text: text, style: style),
      textDirection: TextDirection.ltr,
    )..layout();
    final width = painter.width + 2 * labelPadding;
    return Positioned(
      left: centerX - width / 2,
      top: tipY - labelTip - labelHeight,
      width: width,
      height: labelHeight + labelTip,
      child: Opacity(
        opacity: opacity,
        child: CustomPaint(
          painter: const _LabelPainter(),
          child: Padding(
            padding: const EdgeInsets.only(bottom: labelTip),
            child: Center(child: Text(text, style: style)),
          ),
        ),
      ),
    );
  }

  /// Una finestra ridotta (D-08): la miniatura 57 × 47 centrata nella cella, a filo con la
  /// cima dell'icona, con l'icona dell'app da 18 pt nell'angolo in basso a destra, 4 pt
  /// più in basso della miniatura.
  Widget _windowCell(
    Layout l,
    int index,
    double x,
    double y,
    double side,
    IconSource? source,
    Item item,
  ) {
    final k = side / iconSize;
    final thumbWidth = minimizedThumbWidth * k;
    final thumbHeight = minimizedThumbHeight * k;
    final badge = minimizedBadge * k;
    final left = l.center(index) - thumbWidth / 2;
    final image = item.thumbnail;
    return Positioned(
      key: ValueKey(_key(item)),
      left: left,
      top: y,
      width: thumbWidth,
      height: thumbHeight + minimizedBadgeBelow * k,
      child: Stack(
        clipBehavior: Clip.none,
        children: [
          Positioned(
            left: 0,
            top: 0,
            width: thumbWidth,
            height: thumbHeight,
            child: image == null
                ? const DecoratedBox(
                    decoration: BoxDecoration(color: Color(0x33FFFFFF)),
                  )
                : RawImage(image: image, fit: BoxFit.contain),
          ),
          if (source != null)
            Positioned(
              left: thumbWidth - badge + 2 * k,
              top: thumbHeight - badge + minimizedBadgeBelow * k,
              width: badge,
              height: badge,
              child: ShellIcon(source, side: badge),
            ),
        ],
      ),
    );
  }

  /// L'icona di un'app: a livelli da silica-icons, o dal tema del sistema.
  Widget _icon(
    Layout l,
    int index,
    double height,
    IconSource source,
    Item item,
    bool sliding,
  ) {
    final (x, y, side) = iconRect(l, index, height);
    // Con la pila aperta l'icona della cartella diventa il pulsante che la chiude (D-06).
    final closer = _stackIndex == index ? findSilicaIcon('stack-close') : null;
    final icon = closer != null
        ? ShellIcon(LayeredSource(closer), side: side)
        : ShellIcon(source, side: side);
    return AnimatedPositioned(
      key: ValueKey(_key(item)),
      duration: sliding ? gapSlide : Duration.zero,
      curve: Curves.easeInOut,
      left: x,
      top: y - _bounceOf(item),
      width: side,
      height: side,
      // Premuta: l'icona si scurisce (D-02).
      child: !sliding && _pressed == index
          ? ColorFiltered(
              colorFilter: const ColorFilter.mode(
                pressedShade,
                BlendMode.srcATop,
              ),
              child: icon,
            )
          : icon,
    );
  }

  Widget _label(Layout l, int index, double height, double opacity) {
    final (_, iconTop, _) = iconRect(l, index, height);
    // Durante il rimbalzo l'etichetta sale con l'icona.
    final iconTopY = iconTop - _bounce(index);
    final tipY = iconTopY - labelGap;
    final style = uiText(size: labelFontSize, color: labelText);
    final text = TextPainter(
      text: TextSpan(text: widget.items[index].label, style: style),
      textDirection: TextDirection.ltr,
    )..layout();
    final width = text.width + 2 * labelPadding;
    return Positioned(
      left: l.center(index) - width / 2,
      top: tipY - labelTip - labelHeight,
      width: width,
      height: labelHeight + labelTip,
      child: Opacity(
        opacity: opacity,
        child: CustomPaint(
          painter: const _LabelPainter(),
          child: Padding(
            padding: const EdgeInsets.only(bottom: labelTip),
            child: Center(child: Text(widget.items[index].label, style: style)),
          ),
        ),
      ),
    );
  }
}

class _DockPainter extends CustomPainter {
  _DockPainter(this.items, this.l, this.height);

  final List<Item> items;
  final Layout l;
  final double height;

  @override
  void paint(Canvas canvas, Size size) {
    final (_, by, _, bh) = l.background;
    for (var i = 0; i < items.length; i++) {
      final item = items[i];
      if (item.kind == ItemKind.separator) {
        final x = l.center(i).roundToDouble() - 0.5;
        canvas.drawRect(
          Rect.fromLTWH(x, by + (bh - separatorHeight) / 2, 1, separatorHeight),
          Paint()..color = separatorColor,
        );
        continue;
      }
      if (identical(item, gapItem) || item.minimizedWindow != null) continue;
      final (ix, iy, side) = iconRect(l, i, height);
      if (item.icon == null) {
        paintIcon(canvas, Rect.fromLTWH(ix, iy, side, side), item.label, i);
      }
      if (item.running) {
        canvas.drawCircle(
          Offset(l.center(i), by + bh - indicatorAbove),
          indicatorSize / 2,
          Paint()..color = indicatorColor,
        );
      }
    }
  }

  @override
  bool shouldRepaint(_DockPainter old) => true;
}

class _LabelPainter extends CustomPainter {
  const _LabelPainter();

  @override
  void paint(Canvas canvas, Size size) {
    final body = RRect.fromRectAndRadius(
      Rect.fromLTWH(0.5, 0.5, size.width - 1, labelHeight - 1),
      const Radius.circular(labelHeight / 2),
    );
    final center = size.width / 2;
    final tip = Path()
      ..moveTo(center - labelTipHalfWidth, labelHeight - 1)
      ..lineTo(center, labelHeight + labelTip - 0.5)
      ..lineTo(center + labelTipHalfWidth, labelHeight - 1)
      ..close();
    final shape = Path()
      ..addRRect(body)
      ..addPath(tip, Offset.zero);
    canvas.drawPath(
      shape,
      Paint()
        ..shader = const LinearGradient(
          begin: Alignment.topCenter,
          end: Alignment.bottomCenter,
          // La prima riga chiara (#d7d8de) e quella sotto ancora quasi uguale (215 nella
          // ripresa), poi il corpo.
          colors: [labelBodyTop, labelBodyTop, labelBody, labelBodyBottom],
          stops: [0, 0.07, 0.16, 1],
        ).createShader(Offset.zero & size),
    );
    canvas.drawPath(
      shape,
      Paint()
        ..style = PaintingStyle.stroke
        ..strokeWidth = 1
        ..color = labelBorder,
    );
  }

  @override
  bool shouldRepaint(_LabelPainter old) => false;
}
