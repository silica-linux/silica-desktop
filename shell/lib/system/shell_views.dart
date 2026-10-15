// The whole shell in one engine (#201): the embedder gives every shell layer (wallpaper,
// menu bar, Dock, Faro, switcher, notifications, Control Center) its own view, each still
// its own layer surface; one isolate draws them all, sharing state, settings, theme and the
// image caches.
//
// A view is drawn again only when something in it changed: Flutter composites every view at
// every frame, and the Dock's magnification would otherwise rasterize and present all seven
// layers, most of them transparent, sixty times a second.

// The View widget takes a RenderView and a PipelineOwner of the caller only through these
// parameters: they are what lets a view skip the frames where nothing in it changed.
// ignore_for_file: deprecated_member_use

import 'dart:ui' show FlutterView;

import 'package:flutter/rendering.dart';
import 'package:flutter/widgets.dart';

import 'layer_control.dart';

/// The pipeline of one view, which remembers whether anything in it asked for a frame.
base class LazyPipelineOwner extends PipelineOwner {
  LazyPipelineOwner({super.onSemanticsUpdate});

  /// Something in the view changed since it was last composited (layout, paint, a layer
  /// property, its configuration).
  bool dirty = true;

  @override
  void requestVisualUpdate() {
    dirty = true;
    super.requestVisualUpdate();
  }
}

/// A view that composites (and so rasterizes and presents) only frames where it changed: a
/// view that does not render keeps its last frame on screen.
class LazyRenderView extends RenderView {
  LazyRenderView({required super.view, required this.pipeline});

  final LazyPipelineOwner pipeline;

  /// Frames composited, for the tests.
  @visibleForTesting
  int composited = 0;

  @override
  void compositeFrame() {
    if (!pipeline.dirty) return;
    pipeline.dirty = false;
    composited++;
    super.compositeFrame();
  }
}

/// The views of the shell's layers, each with the component its namespace names.
class ShellViews extends StatefulWidget {
  const ShellViews({
    super.key,
    required this.namespaces,
    required this.builder,
    this.refresh = LayerControl.views,
  });

  /// View id → namespace, as known at startup (the main view's).
  final Map<int, String> namespaces;

  /// The component of a namespace (`menubar` → the menu bar).
  final Widget Function(String namespace) builder;

  /// Asks the embedder for the layers again, when views come and go.
  final Future<Map<int, String>> Function() refresh;

  @override
  State<ShellViews> createState() => _ShellViewsState();
}

class _ShellViewsState extends State<ShellViews> with WidgetsBindingObserver {
  late Map<int, String> _namespaces = widget.namespaces;
  final Map<int, LazyRenderView> _renderViews = {};

  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addObserver(this);
    _refresh();
  }

  @override
  void dispose() {
    WidgetsBinding.instance.removeObserver(this);
    for (final view in _renderViews.values) {
      view.pipeline.dispose();
    }
    super.dispose();
  }

  /// The engine added a view (it changes the metrics): which layer is it?
  @override
  void didChangeMetrics() => _refresh();

  Future<void> _refresh() async {
    final namespaces = await widget.refresh();
    // Rebuilt even with the same layers: a view the embedder named before Dart saw it
    // (the engine adds views asynchronously) gets its View now.
    if (!mounted) return;
    setState(() => _namespaces = namespaces);
  }

  LazyRenderView _renderView(FlutterView view) {
    final known = _renderViews[view.viewId];
    if (known != null && known.flutterView == view) return known;
    final pipeline = LazyPipelineOwner(onSemanticsUpdate: view.updateSemantics);
    return _renderViews[view.viewId] = LazyRenderView(
      view: view,
      pipeline: pipeline,
    );
  }

  @override
  Widget build(BuildContext context) {
    final views = [
      for (final view in WidgetsBinding.instance.platformDispatcher.views)
        if (_namespaces[view.viewId] case final namespace?)
          (view: view, namespace: namespace),
    ];
    return ViewCollection(
      views: [
        for (final (:view, :namespace) in views)
          View(
            key: ValueKey(view.viewId),
            view: view,
            deprecatedDoNotUseWillBeRemovedWithoutNoticePipelineOwner:
                _renderView(view).pipeline,
            deprecatedDoNotUseWillBeRemovedWithoutNoticeRenderView: _renderView(
              view,
            ),
            child: LayerScope(
              control: LayerControl.forView(view.viewId),
              child: widget.builder(namespace),
            ),
          ),
      ],
    );
  }
}
