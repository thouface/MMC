import 'dart:convert';
import 'dart:io';
import 'dart:typed_data';

import '../models/device.dart';
import '../models/screen_frame.dart';

enum MessageType {
  control,
  file,
  heartbeat,
  info,
  screen,
  ack,
}

class NetworkMessage {
  final String msgId;
  final MessageType type;
  final Map<String, dynamic> payload;
  final DateTime timestamp;
  final Uint8List? binary;

  NetworkMessage({
    required this.msgId,
    required this.type,
    required this.payload,
    DateTime? timestamp,
    this.binary,
  }) : timestamp = timestamp ?? DateTime.now();

  Map<String, dynamic> toJson() {
    return {
      'msgId': msgId,
      'type': type.name,
      'payload': payload,
      'timestamp': timestamp.millisecondsSinceEpoch,
    };
  }

  factory NetworkMessage.fromJson(Map<String, dynamic> json) {
    return NetworkMessage(
      msgId: json['msgId'] as String? ?? '',
      type: _parseType(json['type'] as String?),
      payload: Map<String, dynamic>.from(json['payload'] as Map? ?? {}),
      timestamp: DateTime.fromMillisecondsSinceEpoch(
        (json['timestamp'] as int?) ?? 0,
      ),
    );
  }

  static MessageType _parseType(String? value) {
    switch (value) {
      case 'control':
        return MessageType.control;
      case 'file':
        return MessageType.file;
      case 'heartbeat':
        return MessageType.heartbeat;
      case 'info':
        return MessageType.info;
      case 'screen':
        return MessageType.screen;
      case 'ack':
        return MessageType.ack;
      default:
        return MessageType.info;
    }
  }

  Uint8List toWire() {
    final jsonBytes = utf8.encode(jsonEncode(toJson()));
    final length = ByteData(4)..setUint32(0, jsonBytes.length, Endian.big);
    final builder = BytesBuilder(copy: false)
      ..add(length.buffer.asUint8List())
      ..add(jsonBytes);
    if (binary != null) builder.add(binary!);
    return builder.takeBytes();
  }
}

class ControlPayloads {
  ControlPayloads._();

  static Map<String, dynamic> touch(String action, int x, int y, int screenWidth, int screenHeight) {
    return {
      'action': action,
      'x': x,
      'y': y,
      'screenWidth': screenWidth,
      'screenHeight': screenHeight,
    };
  }

  static Map<String, dynamic> key(String keyCode) {
    return {'action': 'key', 'keyCode': keyCode};
  }

  static Map<String, dynamic> fileRequest({
    required String fileName,
    required int fileSize,
    required String mimeType,
    required int chunkSize,
    required int totalChunks,
    required String hash,
  }) {
    return {
      'action': 'request',
      'fileName': fileName,
      'fileSize': fileSize,
      'mimeType': mimeType,
      'chunkSize': chunkSize,
      'totalChunks': totalChunks,
      'hash': hash,
    };
  }

  static Map<String, dynamic> fileChunk({
    required int index,
    required int length,
  }) {
    return {'action': 'chunk', 'index': index, 'length': length};
  }

  static Map<String, dynamic> screenFrame(int width, int height) {
    return {
      'action': 'frame',
      'width': width,
      'height': height,
    };
  }

  static Map<String, dynamic> deviceInfo(Device device) {
    return {
      'deviceId': device.deviceId,
      'name': device.name,
      'type': device.type.name,
      'os': device.os,
    };
  }

  static Map<String, dynamic> ack(String msgId, {bool ok = true}) {
    return {'refMsgId': msgId, 'ok': ok};
  }
}
