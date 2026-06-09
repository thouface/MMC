import 'package:flutter_test/flutter_test.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:mmc/features/device_management/providers/device_list_provider.dart';
import 'package:mmc/models/device.dart';

void main() {
  group('DeviceListNotifier', () {
    test('initial state is empty list', () {
      final container = ProviderContainer();
      final state = container.read(deviceListProvider);

      expect(state, isEmpty);
    });
  });
}