import 'package:flutter_test/flutter_test.dart';
import 'package:mmc/core/constants/app_constants.dart';

void main() {
  group('AppConstants', () {
    test('has correct default values', () {
      expect(AppConstants.appName, 'MMC');
      expect(AppConstants.mdnsServiceType, '_thefool._tcp.local');
      expect(AppConstants.defaultControlPort, 54321);
      expect(AppConstants.defaultFilePort, 54322);
    });

    test('chunk size is 1MB', () {
      expect(AppConstants.defaultChunkSize, 1024 * 1024);
    });

    test('max file size is 2GB', () {
      expect(AppConstants.maxFileSize, 2 * 1024 * 1024 * 1024);
    });

    test('timeout durations are reasonable', () {
      expect(AppConstants.discoveryTimeout.inSeconds, 15);
      expect(AppConstants.heartbeatInterval.inSeconds, 5);
      expect(AppConstants.connectionTimeout.inSeconds, 10);
    });
  });
}