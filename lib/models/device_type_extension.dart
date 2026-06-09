import 'package:flutter/material.dart';

import 'device.dart';

extension DeviceTypeExtension on DeviceType {
  IconData get icon {
    switch (this) {
      case DeviceType.phone:
        return Icons.smartphone;
      case DeviceType.tablet:
        return Icons.tablet;
      case DeviceType.pc:
        return Icons.desktop_windows;
      case DeviceType.other:
        return Icons.devices_other;
    }
  }

  String get displayName {
    switch (this) {
      case DeviceType.phone:
        return '手机';
      case DeviceType.tablet:
        return '平板';
      case DeviceType.pc:
        return '电脑';
      case DeviceType.other:
        return '其他';
    }
  }
}
