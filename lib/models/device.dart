import 'package:hive/hive.dart';

part 'device.g.dart';

@HiveType(typeId: 0)
enum DeviceType {
  @HiveField(0)
  phone,
  @HiveField(1)
  tablet,
  @HiveField(2)
  pc,
  @HiveField(3)
  other,
}

@HiveType(typeId: 1)
enum DeviceStatus {
  @HiveField(0)
  online,
  @HiveField(1)
  offline,
  @HiveField(2)
  connecting,
}

@HiveType(typeId: 2)
class Device extends HiveObject {
  @HiveField(0)
  final String deviceId;

  @HiveField(1)
  String name;

  @HiveField(2)
  final DeviceType type;

  @HiveField(3)
  final String os;

  @HiveField(4)
  String? ip;

  @HiveField(5)
  int? port;

  @HiveField(6)
  int? filePort;

  @HiveField(7)
  DeviceStatus status;

  @HiveField(8)
  DateTime? lastSeen;

  @HiveField(9)
  final DateTime pairedAt;

  Device({
    required this.deviceId,
    required this.name,
    required this.type,
    required this.os,
    this.ip,
    this.port,
    this.filePort,
    this.status = DeviceStatus.offline,
    this.lastSeen,
    required this.pairedAt,
  });

  Device copyWith({
    String? deviceId,
    String? name,
    DeviceType? type,
    String? os,
    String? ip,
    int? port,
    int? filePort,
    DeviceStatus? status,
    DateTime? lastSeen,
    DateTime? pairedAt,
  }) {
    return Device(
      deviceId: deviceId ?? this.deviceId,
      name: name ?? this.name,
      type: type ?? this.type,
      os: os ?? this.os,
      ip: ip ?? this.ip,
      port: port ?? this.port,
      filePort: filePort ?? this.filePort,
      status: status ?? this.status,
      lastSeen: lastSeen ?? this.lastSeen,
      pairedAt: pairedAt ?? this.pairedAt,
    );
  }

  Map<String, dynamic> toMdnsTxt() {
    return {
      'id': deviceId,
      'name': name,
      'type': type.name,
      'os': os,
      'port': port?.toString() ?? '',
      'file_port': filePort?.toString() ?? '',
    };
  }

  factory Device.fromMdnsTxt(Map<String, String> txt, String ip) {
    final now = DateTime.now();
    return Device(
      deviceId: txt['id'] ?? txt['hostname'] ?? ip,
      name: txt['name'] ?? ip,
      type: _parseType(txt['type']),
      os: txt['os'] ?? 'unknown',
      ip: ip,
      port: int.tryParse(txt['port'] ?? ''),
      filePort: int.tryParse(txt['file_port'] ?? ''),
      status: DeviceStatus.online,
      lastSeen: now,
      pairedAt: now,
    );
  }

  static DeviceType _parseType(String? value) {
    switch (value) {
      case 'phone':
        return DeviceType.phone;
      case 'tablet':
        return DeviceType.tablet;
      case 'pc':
        return DeviceType.pc;
      default:
        return DeviceType.other;
    }
  }

  @override
  bool operator ==(Object other) =>
      identical(this, other) ||
      other is Device &&
          runtimeType == other.runtimeType &&
          deviceId == other.deviceId;

  @override
  int get hashCode => deviceId.hashCode;
}
