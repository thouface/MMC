import 'package:flutter_test/flutter_test.dart';
import 'package:mmc/models/device.dart';

void main() {
  group('Device', () {
    test('creates device with required fields', () {
      final device = Device(
        deviceId: 'test-id',
        name: 'Test Device',
        type: DeviceType.phone,
        os: 'android-14',
        pairedAt: DateTime.now(),
      );

      expect(device.deviceId, 'test-id');
      expect(device.name, 'Test Device');
      expect(device.type, DeviceType.phone);
      expect(device.os, 'android-14');
      expect(device.status, DeviceStatus.offline);
    });

    test('copyWith updates specified fields', () {
      final device = Device(
        deviceId: 'test-id',
        name: 'Test Device',
        type: DeviceType.phone,
        os: 'android-14',
        pairedAt: DateTime.now(),
      );

      final updated = device.copyWith(
        name: 'Updated Name',
        status: DeviceStatus.online,
      );

      expect(updated.deviceId, 'test-id');
      expect(updated.name, 'Updated Name');
      expect(updated.status, DeviceStatus.online);
    });

    test('toMdnsTxt returns correct map', () {
      final device = Device(
        deviceId: 'test-id',
        name: 'Test Device',
        type: DeviceType.phone,
        os: 'android-14',
        port: 54321,
        filePort: 54322,
        pairedAt: DateTime.now(),
      );

      final txt = device.toMdnsTxt();

      expect(txt['id'], 'test-id');
      expect(txt['name'], 'Test Device');
      expect(txt['type'], 'phone');
      expect(txt['port'], '54321');
      expect(txt['file_port'], '54322');
    });

    test('fromMdnsTxt creates device from txt record', () {
      final txt = {
        'id': 'remote-id',
        'name': 'Remote Device',
        'type': 'tablet',
        'os': 'ios-17',
        'port': '54321',
        'file_port': '54322',
      };

      final device = Device.fromMdnsTxt(txt, '192.168.1.100');

      expect(device.deviceId, 'remote-id');
      expect(device.name, 'Remote Device');
      expect(device.type, DeviceType.tablet);
      expect(device.os, 'ios-17');
      expect(device.ip, '192.168.1.100');
      expect(device.status, DeviceStatus.online);
    });

    test('equality based on deviceId', () {
      final device1 = Device(
        deviceId: 'same-id',
        name: 'Device 1',
        type: DeviceType.phone,
        os: 'android-14',
        pairedAt: DateTime.now(),
      );

      final device2 = Device(
        deviceId: 'same-id',
        name: 'Device 2',
        type: DeviceType.tablet,
        os: 'ios-17',
        pairedAt: DateTime.now(),
      );

      expect(device1, equals(device2));
      expect(device1.hashCode, equals(device2.hashCode));
    });
  });
}