import 'dart:async';
import 'dart:convert';
import 'dart:io';
import 'dart:typed_data';

import '../core/constants/app_constants.dart';
import '../core/errors/app_exception.dart';
import '../models/device.dart';
import '../protocols/transport_protocol.dart';
import 'package:uuid/uuid.dart';

class Connection {
  Connection(this.socket, this.device, {this.isIncoming = false});

  final Socket socket;
  final Device device;
  final bool isIncoming;

  final StreamController<NetworkMessage> _messageController =
      StreamController<NetworkMessage>.broadcast();
  final List<int> _buffer = <int>[];
  StreamSubscription<List<int>>? _subscription;
  bool _closed = false;

  Stream<NetworkMessage> get messages => _messageController.stream;

  Future<void> listen() async {
    _subscription = socket.listen(
      (data) {
        _buffer.addAll(data);
        _parseMessages();
      },
      onError: (e) {
        if (!_messageController.isClosed) _messageController.addError(e);
      },
      onDone: () {
        if (!_messageController.isClosed) _messageController.close();
      },
      cancelOnError: false,
    );
  }

  void _parseMessages() {
    while (_buffer.length >= 4) {
      final length = ByteData.sublistView(Uint8List.fromList(_buffer.sublist(0, 4))).getUint32(0, Endian.big);
      if (_buffer.length < 4 + length) return;
      final jsonBytes = _buffer.sublist(4, 4 + length);
      _buffer.removeRange(0, 4 + length);
      try {
        final json = jsonDecode(utf8.decode(jsonBytes)) as Map<String, dynamic>;
        _messageController.add(NetworkMessage.fromJson(json));
      } catch (_) {}
    }
  }

  Future<void> send(NetworkMessage message) async {
    if (_closed) throw ConnectionException('连接已关闭');
    socket.add(message.toWire());
    await socket.flush();
  }

  Future<void> close() async {
    if (_closed) return;
    _closed = true;
    await _subscription?.cancel();
    try {
      await socket.close();
    } catch (_) {}
    if (!_messageController.isClosed) await _messageController.close();
  }
}

class ConnectionService {
  final Map<String, Connection> _connections = <String, Connection>{};
  ServerSocket? _server;
  final StreamController<Connection> _incomingController =
      StreamController<Connection>.broadcast();

  Stream<Connection> get incomingConnections => _incomingController.stream;

  List<Connection> get activeConnections => _connections.values.toList();

  Future<void> startServer({int port = AppConstants.defaultControlPort}) async {
    try {
      _server = await ServerSocket.bind(InternetAddress.anyIPv4, port);
      _server!.listen((socket) {
        final connection = Connection(
          socket,
          Device(
            deviceId: const Uuid().v4(),
            name: socket.remoteAddress.address,
            type: DeviceType.other,
            os: 'unknown',
            ip: socket.remoteAddress.address,
            port: socket.remotePort,
            status: DeviceStatus.online,
            lastSeen: DateTime.now(),
            pairedAt: DateTime.now(),
          ),
          isIncoming: true,
        );
        connection.listen();
        _incomingController.add(connection);
      });
    } catch (e) {
      throw ConnectionException('监听端口失败: $port', e);
    }
  }

  Future<Connection> connect(Device device) async {
    if (device.ip == null || device.port == null) {
      throw ConnectionException('设备地址或端口无效');
    }
    try {
      final socket = await Socket.connect(
        device.ip!,
        device.port!,
        timeout: AppConstants.connectionTimeout,
      );
      final connection = Connection(socket, device);
      await connection.listen();
      _connections[device.deviceId] = connection;
      return connection;
    } catch (e) {
      throw ConnectionException('连接设备失败: ${device.ip}', e);
    }
  }

  Future<void> disconnect(String deviceId) async {
    final conn = _connections.remove(deviceId);
    await conn?.close();
  }

  Connection? get(String deviceId) => _connections[deviceId];

  Future<void> stopServer() async {
    await _server?.close();
    _server = null;
    for (final conn in _connections.values) {
      await conn.close();
    }
    _connections.clear();
  }
}
