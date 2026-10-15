// The glass groups of the Dock surface (`Glass.group`). Ossido merges glass only within a
// group (the smooth union of nearby shapes), and a higher group sits on top of a lower one.
// Every logical surface has its own group, so two of them never melt into one blob: the Dock,
// the stack's grid panel, each fan label, the hover label. Only the parts of one surface share
// a group (a panel and its tip). The stack's buttons are not glass (an app tile, flat or
// stroked circles: dock-stacks.md).
//
// The Dock's menus are `MenuHost`s from silica_ui: the menu is group 0 and its submenus 1, 2,
// …, on top of everything else. That is why everything below them is negative (the embedder
// accepts signed groups and sorts them; Ossido only sees the order).

/// Bottom to top.
abstract final class DockGlass {
  /// The Dock's background.
  static const int dock = -1000;

  /// The grid panel (with its tip).
  static const int gridPanel = -900;

  /// The fan's item labels: one group each ([fanLabel]).
  static const int fanLabels = -800;

  /// The hover label and the "Remove" capsule while dragging.
  static const int label = -100;

  /// The glass group of the fan label of item `index` (from the bottom).
  static int fanLabel(int index) =>
      fanLabels + index.clamp(0, label - fanLabels - 1);
}
