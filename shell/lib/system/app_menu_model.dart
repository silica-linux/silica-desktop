// The menus the frontmost app publishes for the menu bar (silica-menu-v1, #25): Ossido checks
// them and hands them to the shell in the format of `silica-menu-v1.xml`. The JSON still comes
// from another process, so it is read defensively: anything unexpected is dropped.

import 'package:flutter/foundation.dart';

/// A shortcut as the app handles it; `meta` is ⌘.
@immutable
class MenuShortcut {
  const MenuShortcut(
    this.key, {
    this.meta = false,
    this.ctrl = false,
    this.alt = false,
    this.shift = false,
  });

  final String key;
  final bool meta;
  final bool ctrl;
  final bool alt;
  final bool shift;

  static MenuShortcut? fromJson(Object? json) {
    if (json is! Map) return null;
    final key = json['key'];
    if (key is! String || key.isEmpty) return null;
    return MenuShortcut(
      key,
      meta: json['meta'] == true,
      ctrl: json['ctrl'] == true,
      alt: json['alt'] == true,
      shift: json['shift'] == true,
    );
  }

  /// As the menus write it: ⌃⌥⇧⌘ then the key ("⌥⌘M", "⌃⌘←").
  String get text {
    final b = StringBuffer();
    if (ctrl) b.write('⌃');
    if (alt) b.write('⌥');
    if (shift) b.write('⇧');
    if (meta) b.write('⌘');
    b.write(keyGlyph(key));
    return b.toString();
  }

  @override
  bool operator ==(Object other) =>
      other is MenuShortcut &&
      other.key == key &&
      other.meta == meta &&
      other.ctrl == ctrl &&
      other.alt == alt &&
      other.shift == shift;

  @override
  int get hashCode => Object.hash(key, meta, ctrl, alt, shift);
}

/// The glyph of a key name of the model.
String keyGlyph(String key) => switch (key.toLowerCase()) {
  'left' || 'arrowleft' => '←',
  'right' || 'arrowright' => '→',
  'up' || 'arrowup' => '↑',
  'down' || 'arrowdown' => '↓',
  'return' || 'enter' => '↩',
  'escape' || 'esc' => '⎋',
  'backspace' => '⌫',
  'delete' => '⌦',
  'tab' => '⇥',
  'space' => 'Space',
  'pageup' => '⇞',
  'pagedown' => '⇟',
  'home' => '↖',
  'end' => '↘',
  _ => key.length == 1 ? key.toUpperCase() : key,
};

/// An item: an action, a submenu ([items] not null) or a separator.
@immutable
class AppMenuItem {
  const AppMenuItem({
    this.id,
    this.label = '',
    this.enabled = true,
    this.checked = false,
    this.role = '',
    this.shortcut,
    this.items,
    this.separator = false,
  });

  const AppMenuItem.separator() : this(separator: true);

  final int? id;
  final String label;
  final bool enabled;
  final bool checked;

  /// A system item (`quit`, `hide`, `minimize`…): the bar runs it itself.
  final String role;
  final MenuShortcut? shortcut;
  final List<AppMenuItem>? items;
  final bool separator;

  static const int _maxDepth = 8;

  static AppMenuItem? fromJson(Object? json, [int depth = 0]) {
    if (json is! Map || depth > _maxDepth) return null;
    if (json['separator'] == true) return const AppMenuItem.separator();
    final id = json['id'];
    final items = json['items'];
    return AppMenuItem(
      id: id is int && id >= 0 ? id : null,
      label: json['label'] is String ? json['label'] as String : '',
      enabled: json['enabled'] != false,
      checked: json['checked'] == true,
      role: json['role'] is String ? json['role'] as String : '',
      shortcut: MenuShortcut.fromJson(json['shortcut']),
      items: items is List ? _items(items, depth + 1) : null,
    );
  }

  static List<AppMenuItem> _items(List<Object?> list, int depth) => [
    for (final item in list)
      if (AppMenuItem.fromJson(item, depth) case final parsed?) parsed,
  ];
}

/// A top-level menu.
@immutable
class AppMenu {
  const AppMenu({required this.label, this.role = '', this.items = const []});

  final String label;

  /// `app`, `window`, `help` or empty.
  final String role;
  final List<AppMenuItem> items;

  static AppMenu? fromJson(Object? json) {
    if (json is! Map) return null;
    final items = json['items'];
    return AppMenu(
      label: json['label'] is String ? json['label'] as String : '',
      role: json['role'] is String ? json['role'] as String : '',
      items: items is List ? AppMenuItem._items(items, 1) : const [],
    );
  }
}

/// The frontmost app's menus as Ossido announced them.
@immutable
class AppMenuState {
  const AppMenuState({this.appId = '', this.serial = 0, this.menus});

  /// The frontmost app (empty with none).
  final String appId;

  /// The app's menu object, for the choice; 0 without menus.
  final int serial;

  /// Null when the app published no menus: the bar shows its generic ones.
  final List<AppMenu>? menus;

  bool get hasMenus => serial != 0 && menus != null;

  static AppMenuState fromJson(Object? json) {
    if (json is! Map) return const AppMenuState();
    final serial = json['serial'];
    final model = json['model'];
    final menus = model is Map ? model['menus'] : null;
    return AppMenuState(
      appId: json['app_id'] is String ? json['app_id'] as String : '',
      serial: serial is int ? serial : 0,
      menus: menus is List
          ? [
              for (final menu in menus)
                if (AppMenu.fromJson(menu) case final parsed?) parsed,
            ]
          : null,
    );
  }
}
