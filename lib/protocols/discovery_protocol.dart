import 'dart:async';
import 'dart:io';

import '../models/device.dart';

/// 设备发现协议 —— mDNS 抽象层，不同平台通过 Platform Channel 实现，
/// 此处提供 Dart 端的接口定义与 UDP 广播回退方案。
abstract class DiscoveryProtocol {
  final String serviceType;
  final int port;

  DiscoveryProtocol(this.serviceType, this.port);

  Future<void> startAdvertise(Device device);

  Future<void> stopAdvertise();

  Future<void> startDiscovery();

  Future<void> stopDiscovery();

  Stream<List<Device>> get discoveredDevices;
}

/// 基于 UDP 广播的简易回退发现协议。
/// 当平台 mDNS 不可用时，使用 UDP 在局域网内广播设备信息。
class UdpDiscoveryProtocol extends DiscoveryProtocol {
  UdpDiscoveryProtocol({
    required String serviceType,
    required int port,
    required this.device,
  }) : super(serviceType, port);

  final Device device;

  RawDatagramSocket? _socket;
  Timer? _advertiseTimer;
  final Map<String, Device> _discovered = <String, Device>{};
  final _controller = StreamController<List<Device>>.broadcast();
  bool _running = false;

  @override
  Future<void> startAdvertise(Device device) async {
    try {
      _socket ??= await RawDatagramSocket.bind(InternetAddress.anyIPv4, port);
      _socket!.broadcastEnabled = true;
      _socket!.listen((event) {
        if (event == RawSocketEvent.read) {
          final datagram = _socket!.receive();
          if (datagram == null) return;
          _handleIncoming(datagram);
        }
      });
      _running = true;
      _advertiseTimer?.cancel();
      _advertiseTimer = Timer.periodic(const Duration(seconds: 3), (_) => _sendAdvertise());
      _sendAdvertise();
    } catch (_) {}
  }

  @override
  Future<void> stopAdvertise() async {
    _advertiseTimer?.cancel();
    _advertiseTimer = null;
  }

  @override
  Future<void> startDiscovery() async {
    _socket ??= await RawDatagramSocket.bind(InternetAddress.anyIPv4, 0);
    _socket!.broadcastEnabled = true;
    _socket!.listen((event) {
      if (event == RawSocketEvent.read) {
        final datagram = _socket!.receive();
        if (datagram == null) return;
        _handleIncoming(datagram);
      }
    });
    _running = true;
  }

  @override
  Future<void> stopDiscovery() async {
    _running = false;
    _socket?.close();
    _socket = null;
    _advertiseTimer?.cancel();
    _advertiseTimer = null;
    await _controller.close();
  }

  @override
  Stream<List<Device>> get discoveredDevices => _controller.stream;

  void _sendAdvertise() {
    if (_socket == null || !_running) return;
    try {
      final data = '${device.deviceId}|${device.name}|${device.type.name}|${device.os}|${device.port ?? port}|${device.filePort ?? port}';
      _socket!.send(
        data.codeUnits, InternetAddress('255.255.255.255'), port);
    } catch (_) {}
  }

  void _handleIncoming(Datagram datagram) {
    try {
      final text = String.fromCharCodes(datagram.data);
      final parts = text.split('|');
      if (parts.length < 6) return;
      final id = parts[0];
      if (id == device.deviceId) return;
      final discovered = Device(
        deviceId: id,
        name: parts[1],
        type: DeviceType.values.firstWhere(
          (e) => e.name == parts[2],
          orElse: () => DeviceType.other,
        ),
        os: parts[3],
        ip: datagram.address.address,
        port: int.tryParse(parts[4]),
        filePort: int.tryParse(parts[5]),
        status: DeviceStatus.online,
        lastSeen: DateTime.now(),
        pairedAt: DateTime.now(),
      );
      _discovered[id] = discovered;
      if (!_controller.isClosed) _controller.add(_discovered.values.toList());
    } catch (_) {}
  }
}
