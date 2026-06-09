import 'dart:async';

import 'package:device_info_plus/device_info_plus.dart';
import 'package:network_info_plus/network_info_plus.dart';
import 'package:uuid/uuid.dart';

import '../core/constants/app_constants.dart';
import '../core/errors/app_exception.dart';
import '../models/device.dart';
import '../protocols/discovery_protocol.dart';
import 'device_repository.dart';

class DiscoveryService {
  DiscoveryService(this._protocol);

  final DiscoveryProtocol _protocol;
  final _controller = StreamController<List<Device>>.broadcast();
  Device? _self;
  bool _isDiscovering = false;

  static Future<DiscoveryService> init() async {
    final self = await _buildSelfDevice();
    final protocol = UdpDiscoveryProtocol(
      serviceType: AppConstants.mdnsServiceType,
      port: AppConstants.defaultControlPort,
      device: self,
    );
    return DiscoveryService(protocol).._self = self;
  }

  Device? get self => _self;

  bool get isDiscovering => _isDiscovering;

  Stream<List<Device>> get discoveredDevices => _controller.stream;

  Future<void> startDiscovery() async {
    if (_isDiscovering) return;
    try {
      _isDiscovering = true;
      await _protocol.startDiscovery();
      await _protocol.startAdvertise(_self!);
      _protocol.discoveredDevices.listen(
        (devices) {
          if (!_controller.isClosed) _controller.add(devices);
        },
        onError: (e) {
          if (!_controller.isClosed) _controller.addError(e);
        },
      );
    } catch (e) {
      _isDiscovering = false;
      throw DiscoveryException('设备发现失败', e);
    }
  }

  Future<void> stopDiscovery() async {
    _isDiscovering = false;
    await _protocol.stopDiscovery();
    await _protocol.stopAdvertise();
  }

  Future<void> pair(Device device) async {
    device = device.copyWith(
      status: DeviceStatus.online,
      lastSeen: DateTime.now(),
      pairedAt: DateTime.now(),
    );
    await DeviceRepository.instance.save(device);
  }

  Future<Device> manualPair(String ip, int port, String name) async {
    final device = Device(
      deviceId: const Uuid().v4(),
      name: name,
      type: DeviceType.other,
      os: 'unknown',
      ip: ip,
      port: port,
      filePort: AppConstants.defaultFilePort,
      status: DeviceStatus.online,
      lastSeen: DateTime.now(),
      pairedAt: DateTime.now(),
    );
    await DeviceRepository.instance.save(device);
    return device;
  }

  static Future<Device> _buildSelfDevice() async {
    final info = DeviceInfoPlugin();
    final plugin = NetworkInfo();
    String name = 'Unknown';
    String os = 'unknown';
    DeviceType type = DeviceType.other;

    try {
      final android = await info.androidInfo;
      name = android.model;
      os = 'android-${android.version.release}';
      type = DeviceType.phone;
    } catch (_) {
      try {
        final ios = await info.iosInfo;
        name = ios.name;
        os = 'ios-${ios.systemVersion}';
        type = DeviceType.phone;
      } catch (_) {}
    }

    String? ip;
    try {
      ip = await plugin.getWifiIP();
    } catch (_) {}

    return Device(
      deviceId: const Uuid().v4(),
      name: name,
      type: type,
      os: os,
      ip: ip,
      port: AppConstants.defaultControlPort,
      filePort: AppConstants.defaultFilePort,
      status: DeviceStatus.online,
      lastSeen: DateTime.now(),
      pairedAt: DateTime.now(),
    );
  }

  Future<void> dispose() async {
    await stopDiscovery();
    await _controller.close();
  }
}
