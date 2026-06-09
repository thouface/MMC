import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../models/device.dart';
import '../../services/device_repository.dart';

class DeviceListNotifier extends Notifier<List<Device>> {
  @override
  List<Device> build() {
    _load();
    return [];
  }

  Future<void> _load() async {
    state = await DeviceRepository.instance.all();
  }

  Future<void> refresh() => _load();

  Future<void> rename(String deviceId, String newName) async {
    final device = await DeviceRepository.instance.find(deviceId);
    if (device == null) return;
    await DeviceRepository.instance.save(device.copyWith(name: newName));
    await refresh();
  }

  Future<void> remove(String deviceId) async {
    await DeviceRepository.instance.remove(deviceId);
    await refresh();
  }
}

final deviceListProvider = NotifierProvider<DeviceListNotifier, List<Device>>(
  DeviceListNotifier.new,
);
