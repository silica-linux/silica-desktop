import 'package:flutter/material.dart' show Colors;
import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:silica_shell/system/layer_control.dart';
import 'package:silica_shell/system/shell_channel.dart';
import 'package:silica_shell/system/shell_views.dart';

/// A component that counts its taps, to change something in its view on demand.
class Counter extends StatefulWidget {
  const Counter({super.key, required this.name});

  final String name;

  @override
  State<Counter> createState() => CounterState();
}

class CounterState extends State<Counter> {
  int taps = 0;
  LayerControl? layer;

  @override
  void initState() {
    super.initState();
    layer = LayerControl.of(context);
  }

  @override
  Widget build(BuildContext context) => Directionality(
    textDirection: TextDirection.ltr,
    child: ColoredBox(color: Colors.black, child: Text('${widget.name} $taps')),
  );
}

void main() {
  testWidgets('every layer view draws the component its namespace names', (
    tester,
  ) async {
    final id = tester.view.viewId;
    await tester.pumpWidget(
      wrapWithView: false,
      ShellViews(
        namespaces: {id: 'menubar'},
        refresh: () async => {id: 'menubar'},
        builder: (namespace) => Counter(name: namespace),
      ),
    );
    expect(find.text('menubar 0'), findsOneWidget);
    // The component reaches its own layer: the calls name its view.
    final state = tester.state<CounterState>(find.byType(Counter));
    expect(state.layer, LayerControl.forView(id));
  });

  testWidgets('a view is composited again only when something in it changed', (
    tester,
  ) async {
    final id = tester.view.viewId;
    await tester.pumpWidget(
      wrapWithView: false,
      ShellViews(
        namespaces: {id: 'dock'},
        refresh: () async => {id: 'dock'},
        builder: (namespace) => Counter(name: namespace),
      ),
    );
    await tester.pump();
    final view = tester.binding.renderViews.whereType<LazyRenderView>().single;
    final before = view.composited;
    // Frames with nothing new in this view do not draw it.
    tester.binding.scheduleFrame();
    await tester.pump();
    tester.binding.scheduleFrame();
    await tester.pump();
    expect(view.composited, before);
    // A change in it does.
    final state = tester.state<CounterState>(find.byType(Counter));
    // ignore: invalid_use_of_protected_member
    state.setState(() => state.taps++);
    await tester.pump();
    expect(view.composited, before + 1);
    expect(find.text('dock 1'), findsOneWidget);
  });

  testWidgets(
    'layer calls carry the view only for a layer of a shared engine',
    (tester) async {
      final calls = <MethodCall>[];
      tester.binding.defaultBinaryMessenger.setMockMethodCallHandler(
        LayerControl.channel,
        (call) async {
          calls.add(call);
          return true;
        },
      );
      addTearDown(
        () => tester.binding.defaultBinaryMessenger.setMockMethodCallHandler(
          LayerControl.channel,
          null,
        ),
      );
      await LayerControl.main.keyboard('exclusive');
      await const LayerControl.forView(3).keyboard('none');
      await const LayerControl.forView(3).transient();
      expect(calls[0].arguments, {'mode': 'exclusive'});
      expect(calls[1].arguments, {'mode': 'none', 'viewId': 3});
      expect((calls[2].arguments as Map)['viewId'], 3);
    },
  );

  testWidgets('the layers of the shared engine are read by namespace', (
    tester,
  ) async {
    tester.binding.defaultBinaryMessenger.setMockMethodCallHandler(
      LayerControl.channel,
      (call) async => call.method == 'views'
          ? [
              {'viewId': 0, 'namespace': 'wallpaper'},
              {'viewId': 2, 'namespace': 'dock'},
              {'viewId': 'x', 'namespace': 'bad'},
              'garbage',
            ]
          : null,
    );
    addTearDown(
      () => tester.binding.defaultBinaryMessenger.setMockMethodCallHandler(
        LayerControl.channel,
        null,
      ),
    );
    expect(await LayerControl.views(), {0: 'wallpaper', 2: 'dock'});
  });

  testWidgets('a dismissal reaches only the layer it names', (tester) async {
    Future<void> dismiss(Object? args) =>
        tester.binding.defaultBinaryMessenger.handlePlatformMessage(
          ShellChannel.channel.name,
          const JSONMethodCodec().encodeMethodCall(
            MethodCall('transientDismissed', args),
          ),
          (_) {},
        );
    ShellChannel.instance; // Listens on the channel.
    final menubar = <void>[];
    final dock = <void>[];
    final single = <void>[];
    final subscriptions = [
      const LayerControl.forView(1).transientDismissed.listen(menubar.add),
      const LayerControl.forView(2).transientDismissed.listen(dock.add),
      LayerControl.main.transientDismissed.listen(single.add),
    ];
    await dismiss({'viewId': 2});
    await dismiss(null);
    await tester.pump();
    expect(menubar.length, 1);
    expect(dock.length, 2);
    expect(single.length, 2);
    for (final subscription in subscriptions) {
      // ignore: discarded_futures
      subscription.cancel();
    }
  });
}
