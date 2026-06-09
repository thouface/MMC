import 'dart:async';
import 'dart:typed_data';

import 'package:uuid/uuid.dart';

import '../core/errors/app_exception.dart';
import '../models/device.dart';
import '../models/screen_frame.dart';
import '../protocols/transport_protocol.dart';
import 'connection_service.dart';

class ControlService {
  ControlService(this._connectionService);

  final ConnectionService _connectionService;
  final Map<String, StreamSubscription<NetworkMessage>> _subs = {};
  final Map<String, StreamController<ScreenFrame>> _frameControllers = {};

  Stream<ScreenFrame> screenFrames(String deviceId) {
    return _frameControllers[deviceId]?.stream ??
        (StreamController<ScreenFrame>.broadcast()..close()).stream;
  }

  Future<void> startControl(Device target) async {
    var conn = _connectionService.get(target.deviceId);
    conn ??= await _connectionService.connect(target);

    _frameControllers[target.deviceId] ??= StreamController<ScreenFrame>.broadcast();

    _subs[target.deviceId] = conn.messages.listen(
      (msg) {
        if (msg.type == MessageType.screen) {
          final payload = msg.payload;
          final width = payload['width'] as int? ?? 0;
          final height = payload['height'] as int? ?? 0;
          if (msg.binary != null && width > 0 && height > 0) {
            _frameControllers[target.deviceId]?.add(
              ScreenFrame(
                image: msg.binary!,
                width: width,
                height: height,
                timestamp: msg.timestamp,
              ),
            );
          }
        }
      },
      onError: (e) {
        _frameControllers[target.deviceId]?.addError(e);
      },
    );
  }

  Future<void> stopControl(String deviceId) async {
    await _subs.remove(deviceId)?.cancel();
    await _frameControllers.remove(deviceId)?.close();
    await _connectionService.disconnect(deviceId);
  }

  Future<void> sendTouch({
    required String deviceId,
    required int x,
    required int y,
    required int screenWidth,
    required int screenHeight,
    String action = 'touch_down',
  }) async {
    final conn = _connectionService.get(deviceId);
    if (conn == null) throw ControlException('未连接到设备');
    await conn.send(
      NetworkMessage(
        msgId: const Uuid().v4(),
        type: MessageType.control,
        payload: ControlPayloads.touch(action, x, y, screenWidth, screenHeight),
      ),
    );
  }

  Future<void> sendKey(String deviceId, String keyCode) async {
    final conn = _connectionService.get(deviceId);
    if (conn == null) throw ControlException('未连接到设备');
    await conn.send(
      NetworkMessage(
        msgId: const Uuid().v4(),
        type: MessageType.control,
        payload: ControlPayloads.key(keyCode),
      ),
    );
  }

  Future<void> pushFrame({
    required String deviceId,
    required Uint8List image,
    required int width,
    required int height,
  }) async {
    final conn = _connectionService.get(deviceId);
    if (conn == null) return;
    await conn.send(
      NetworkMessage(
        msgId: const Uuid().v4(),
        type: MessageType.screen,
        payload: ControlPayloads.screenFrame(width, height),
        binary: image,
      ),
    );
  }
}
