import 'package:flutter_test/flutter_test.dart';
import 'package:mmc/models/transfer_task.dart';

void main() {
  group('TransferTask', () {
    test('creates task with required fields', () {
      final task = TransferTask(
        taskId: 'task-id',
        fileName: 'test.jpg',
        fileSize: 1024000,
        mimeType: 'image/jpeg',
        targetDeviceId: 'device-id',
        direction: TransferDirection.send,
      );

      expect(task.taskId, 'task-id');
      expect(task.fileName, 'test.jpg');
      expect(task.fileSize, 1024000);
      expect(task.direction, TransferDirection.send);
      expect(task.status, TransferStatus.pending);
      expect(task.transferredBytes, 0);
    });

    test('progress calculation', () {
      final task = TransferTask(
        taskId: 'task-id',
        fileName: 'test.jpg',
        fileSize: 1000,
        mimeType: 'image/jpeg',
        targetDeviceId: 'device-id',
        direction: TransferDirection.send,
        transferredBytes: 500,
      );

      expect(task.progress, 0.5);
    });

    test('progress returns 1.0 when done even if fileSize is 0', () {
      final task = TransferTask(
        taskId: 'task-id',
        fileName: 'test.jpg',
        fileSize: 0,
        mimeType: 'image/jpeg',
        targetDeviceId: 'device-id',
        direction: TransferDirection.send,
        status: TransferStatus.done,
      );

      expect(task.progress, 1.0);
    });

    test('remainingBytes calculation', () {
      final task = TransferTask(
        taskId: 'task-id',
        fileName: 'test.jpg',
        fileSize: 1000,
        mimeType: 'image/jpeg',
        targetDeviceId: 'device-id',
        direction: TransferDirection.send,
        transferredBytes: 300,
      );

      expect(task.remainingBytes, 700);
    });

    test('copyWith updates fields', () {
      final task = TransferTask(
        taskId: 'task-id',
        fileName: 'test.jpg',
        fileSize: 1000,
        mimeType: 'image/jpeg',
        targetDeviceId: 'device-id',
        direction: TransferDirection.send,
      );

      final updated = task.copyWith(
        status: TransferStatus.transferring,
        transferredBytes: 500,
      );

      expect(updated.status, TransferStatus.transferring);
      expect(updated.transferredBytes, 500);
      expect(updated.taskId, 'task-id');
    });
  });
}