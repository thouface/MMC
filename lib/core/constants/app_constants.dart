class AppConstants {
  AppConstants._();

  static const String appName = 'MMC';
  static const String appVersion = '1.0.0';

  static const String mdnsServiceType = '_thefool._tcp.local';
  static const int defaultControlPort = 54321;
  static const int defaultFilePort = 54322;

  static const Duration discoveryTimeout = Duration(seconds: 15);
  static const Duration heartbeatInterval = Duration(seconds: 5);
  static const Duration connectionTimeout = Duration(seconds: 10);

  static const int defaultChunkSize = 1024 * 1024; // 1MB
  static const int maxFileSize = 2 * 1024 * 1024 * 1024; // 2GB

  static const String hiveBoxDevices = 'paired_devices';
  static const String hiveBoxTransfers = 'transfer_history';
  static const String hiveBoxSettings = 'app_settings';
}
