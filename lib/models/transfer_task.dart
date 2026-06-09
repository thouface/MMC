import 'package:hive/hive.dart';

part 'transfer_task.g.dart';

@HiveType(typeId: 3)
enum TransferDirection {
  @HiveField(0)
  send,
  @HiveField(1)
  receive,
}

@HiveType(typeId: 4)
enum TransferStatus {
  @HiveField(0)
  pending,
  @HiveField(1)
  transferring,
  @HiveField(2)
  done,
  @HiveField(3)
  failed,
  @HiveField(4)
  canceled,
}

@HiveType(typeId: 5)
class TransferTask extends HiveObject {
  @HiveField(0)
  final String taskId;

  @HiveField(1)
  final String fileName;

  @HiveField(2)
  final int fileSize;

  @HiveField(3)
  final String mimeType;

  @HiveField(4)
  final String targetDeviceId;

  @HiveField(5)
  final TransferDirection direction;

  @HiveField(6)
  TransferStatus status;

  @HiveField(7)
  int transferredBytes;

  @HiveField(8)
  String? localPath;

  @HiveField(9)
  String? errorMessage;

  @HiveField(10)
  final DateTime createdAt;

  @HiveField(11)
  DateTime? updatedAt;

  TransferTask({
    required this.taskId,
    required this.fileName,
    required this.fileSize,
    required this.mimeType,
    required this.targetDeviceId,
    required this.direction,
    this.status = TransferStatus.pending,
    this.transferredBytes = 0,
    this.localPath,
    this.errorMessage,
    DateTime? createdAt,
    this.updatedAt,
  }) : createdAt = createdAt ?? DateTime.now();

  double get progress {
    if (fileSize <= 0) return status == TransferStatus.done ? 1.0 : 0.0;
    return (transferredBytes / fileSize).clamp(0.0, 1.0);
  }

  int get remainingBytes => (fileSize - transferredBytes).clamp(0, fileSize);

  TransferTask copyWith({
    String? taskId,
    String? fileName,
    int? fileSize,
    String? mimeType,
    String? targetDeviceId,
    TransferDirection? direction,
    TransferStatus? status,
    int? transferredBytes,
    String? localPath,
    String? errorMessage,
    DateTime? createdAt,
    DateTime? updatedAt,
  }) {
    return TransferTask(
      taskId: taskId ?? this.taskId,
      fileName: fileName ?? this.fileName,
      fileSize: fileSize ?? this.fileSize,
      mimeType: mimeType ?? this.mimeType,
      targetDeviceId: targetDeviceId ?? this.targetDeviceId,
      direction: direction ?? this.direction,
      status: status ?? this.status,
      transferredBytes: transferredBytes ?? this.transferredBytes,
      localPath: localPath ?? this.localPath,
      errorMessage: errorMessage ?? this.errorMessage,
      createdAt: createdAt ?? this.createdAt,
      updatedAt: updatedAt ?? DateTime.now(),
    );
  }
}
