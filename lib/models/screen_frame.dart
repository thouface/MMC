import 'dart:typed_data';

class ScreenFrame {
  final Uint8List image;
  final int width;
  final int height;
  final DateTime timestamp;

  ScreenFrame({
    required this.image,
    required this.width,
    required this.height,
    required this.timestamp,
  });

  double get aspectRatio => height > 0 ? width / height : 1.0;
}
