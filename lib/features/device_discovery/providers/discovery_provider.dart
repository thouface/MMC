import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../models/device.dart';
import '../../services/discovery_service.dart';

class DiscoveryNotifier extends Notifier<AsyncValue<List<Device>>> {
  DiscoveryService? _service;

  @override
  AsyncValue<List<Device>> build() {
    ref.onDispose(() => _service?.stopDiscovery());
    return const AsyncValue.data([]);
  }

  Future<void> start() async {
    state = const AsyncValue.loading();
    try {
      _service ??= await DiscoveryService.init();
      await _service!.startDiscovery();
      _service!.discoveredDevices.listen(
        (devices) {
          state = AsyncValue.data(devices);
        },
        onError: (e) {
          state = AsyncValue.error(e, StackTrace.current);
        },
      );
    } catch (e, st) {
      state = AsyncValue.error(e, st);
    }
  }

  Future<void> stop() async {
    await _service?.stopDiscovery();
  }

  Future<void> pair(Device device) async {
    await _service?.pair(device);
  }

  Future<Device> manualPair(String ip, int port, String name) async {
    _service ??= await DiscoveryService.init();
    return _service!.manualPair(ip, port, name);
  }
}

final discoveryProvider =
    NotifierProvider<DiscoveryNotifier, AsyncValue<List<Device>>>(
  DiscoveryNotifier.new,
);
