import 'package:hive_flutter/hive_flutter.dart';

import '../core/constants/app_constants.dart';
import '../models/device.dart';

class DeviceRepository {
  DeviceRepository._();

  static final DeviceRepository instance = DeviceRepository._();

  Box<Device>? _box;

  Future<Box<Device>> get box async {
    return _box ??= await Hive.openBox<Device>(AppConstants.hiveBoxDevices);
  }

  Future<List<Device>> all() async {
    final b = await box;
    return b.values.toList();
  }

  Future<Device?> find(String deviceId) async {
    final b = await box;
    try {
      return b.values.firstWhere((d) => d.deviceId == deviceId);
    } on StateError {
      return null;
    }
  }

  Future<void> save(Device device) async {
    final b = await box;
    final existing = await find(device.deviceId);
    if (existing != null) {
      final key = b.keys.firstWhere((k) => b.get(k)?.deviceId == device.deviceId);
      await b.put(key, device);
    } else {
      await b.add(device);
    }
  }

  Future<void> remove(String deviceId) async {
    final b = await box;
    final key = b.keys.firstWhere(
      (k) => b.get(k)?.deviceId == deviceId,
      orElse: () => null,
    );
    if (key != null) await b.delete(key);
  }

  Future<void> clear() async {
    final b = await box;
    await b.clear();
  }
}
