import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../models/device.dart';
import '../../models/screen_frame.dart';
import '../../services/connection_service.dart';
import '../../services/control_service.dart';

class RemoteControlState {
  final bool isConnected;
  final bool isControlling;
  final ScreenFrame? frame;
  final String? error;

  RemoteControlState({
    this.isConnected = false,
    this.isControlling = false,
    this.frame,
    this.error,
  });
}

class RemoteControlNotifier extends Notifier<RemoteControlState> {
  final _conn = ConnectionService();
  late final ControlService _ctrl = ControlService(_conn);

  @override
  RemoteControlState build() {
    ref.onDispose(() async => await _conn.stopServer());
    return RemoteControlState();
  }

  Future<void> connect(Device device) async {
    try {
      state = RemoteControlState(isConnected: false);
      await _ctrl.startControl(device);
      _ctrl.screenFrames(device.deviceId).listen(
        (frame) {
          state = RemoteControlState(
            isConnected: true,
            isControlling: true,
            frame: frame,
          );
        },
        onError: (e) {
          state = RemoteControlState(error: e.toString());
        },
      );
    } catch (e) {
      state = RemoteControlState(error: e.toString());
    }
  }

  Future<void> disconnect(Device device) async {
    await _ctrl.stopControl(device.deviceId);
    state = RemoteControlState();
  }

  Future<void> touch(Device device, double x, double y, double w, double h, String action) async {
    await _ctrl.sendTouch(
      deviceId: device.deviceId,
      x: x.toInt(),
      y: y.toInt(),
      screenWidth: w.toInt(),
      screenHeight: h.toInt(),
      action: action,
    );
  }

  Future<void> key(Device device, String keyCode) async {
    await _ctrl.sendKey(device.deviceId, keyCode);
  }
}

final remoteControlProvider =
    NotifierProvider<RemoteControlNotifier, RemoteControlState>(
  RemoteControlNotifier.new,
);
