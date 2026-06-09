import 'package:flutter_test/flutter_test.dart';
import 'package:mmc/protocols/transport_protocol.dart';

void main() {
  group('NetworkMessage', () {
    test('creates message with all fields', () {
      final msg = NetworkMessage(
        msgId: 'msg-123',
        type: MessageType.control,
        payload: {'action': 'touch_down', 'x': 100, 'y': 200},
      );

      expect(msg.msgId, 'msg-123');
      expect(msg.type, MessageType.control);
      expect(msg.payload['action'], 'touch_down');
    });

    test('toJson and fromJson roundtrip', () {
      final original = NetworkMessage(
        msgId: 'msg-123',
        type: MessageType.control,
        payload: {'action': 'touch_down', 'x': 100, 'y': 200},
      );

      final json = original.toJson();
      final restored = NetworkMessage.fromJson(json);

      expect(restored.msgId, original.msgId);
      expect(restored.type, original.type);
      expect(restored.payload['action'], original.payload['action']);
    });

    test('toWire produces bytes with length prefix', () {
      final msg = NetworkMessage(
        msgId: 'msg-123',
        type: MessageType.heartbeat,
        payload: {},
      );

      final wire = msg.toWire();

      // First 4 bytes are length prefix
      expect(wire.length, greaterThan(4));
    });
  });

  group('ControlPayloads', () {
    test('touch payload format', () {
      final payload = ControlPayloads.touch('touch_down', 100, 200, 1080, 1920);

      expect(payload['action'], 'touch_down');
      expect(payload['x'], 100);
      expect(payload['y'], 200);
      expect(payload['screenWidth'], 1080);
      expect(payload['screenHeight'], 1920);
    });

    test('key payload format', () {
      final payload = ControlPayloads.key('BACK');

      expect(payload['action'], 'key');
      expect(payload['keyCode'], 'BACK');
    });

    test('fileRequest payload format', () {
      final payload = ControlPayloads.fileRequest(
        fileName: 'test.jpg',
        fileSize: 1024000,
        mimeType: 'image/jpeg',
        chunkSize: 1048576,
        totalChunks: 1,
        hash: 'abc123',
      );

      expect(payload['action'], 'request');
      expect(payload['fileName'], 'test.jpg');
      expect(payload['fileSize'], 1024000);
    });

    test('ack payload format', () {
      final payload = ControlPayloads.ack('msg-123', ok: true);

      expect(payload['refMsgId'], 'msg-123');
      expect(payload['ok'], true);
    });
  });
}