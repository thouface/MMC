class AppException implements Exception {
  final String message;
  final Object? cause;

  AppException(this.message, [this.cause]);

  @override
  String toString() => cause == null ? message : '$message: $cause';
}

class DiscoveryException extends AppException {
  DiscoveryException(super.message, [super.cause]);
}

class ConnectionException extends AppException {
  ConnectionException(super.message, [super.cause]);
}

class TransferException extends AppException {
  TransferException(super.message, [super.cause]);
}

class ControlException extends AppException {
  ControlException(super.message, [super.cause]);
}
