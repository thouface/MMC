import 'dart:typed_data';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../providers/remote_control_provider.dart';
import '../../device_management/providers/device_list_provider.dart';

class RemoteControlPage extends ConsumerStatefulWidget {
  const RemoteControlPage({super.key, required this.deviceId});

  final String deviceId;

  @override
  ConsumerState<RemoteControlPage> createState() => _RemoteControlPageState();
}

class _RemoteControlPageState extends ConsumerState<RemoteControlPage> {
  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addPostFrameCallback((_) {
      final devices = ref.read(deviceListProvider);
      final device = devices.firstWhere((d) => d.deviceId == widget.deviceId);
      ref.read(remoteControlProvider.notifier).connect(device);
    });
  }

  @override
  Widget build(BuildContext context) {
    final state = ref.watch(remoteControlProvider);
    final devices = ref.watch(deviceListProvider);
    final device = devices.firstWhere((d) => d.deviceId == widget.deviceId);

    return Scaffold(
      appBar: AppBar(title: Text('控制 ${device.name}')),
      body: Column(
        children: [
          Expanded(
            child: Center(
              child: state.error != null
                  ? Text(state.error!)
                  : state.frame != null
                      ? _ScreenWidget(frame: state.frame!)
                      : const CircularProgressIndicator(),
            ),
          ),
          SafeArea(
            child: Padding(
              padding: const EdgeInsets.all(16),
              child: Row(
                mainAxisAlignment: MainAxisAlignment.spaceEvenly,
                children: [
                  ElevatedButton(
                    onPressed: () =>
                        ref.read(remoteControlProvider.notifier).key(device, 'BACK'),
                    child: const Text('返回'),
                  ),
                  ElevatedButton(
                    onPressed: () =>
                        ref.read(remoteControlProvider.notifier).key(device, 'HOME'),
                    child: const Text('Home'),
                  ),
                  ElevatedButton(
                    onPressed: () =>
                        ref.read(remoteControlProvider.notifier).disconnect(device),
                    child: const Text('断开'),
                  ),
                ],
              ),
            ),
          ),
        ],
      ),
    );
  }
}

class _ScreenWidget extends StatelessWidget {
  const _ScreenWidget({required this.frame});
  final ScreenFrame frame;

  @override
  Widget build(BuildContext context) {
    return AspectRatio(
      aspectRatio: frame.aspectRatio,
      child: Container(
        color: Colors.black,
        child: Image.memory(frame.image, fit: BoxFit.contain),
      ),
    );
  }
}
