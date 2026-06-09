import 'dart:async';
import 'dart:io';
import 'dart:typed_data';

import 'package:uuid/uuid.dart';

import '../core/constants/app_constants.dart';
import '../core/errors/app_exception.dart';
import '../models/device.dart';
import '../models/transfer_task.dart';
import 'connection_service.dart';
import 'device_repository.dart';
import 'package:hive_flutter/hive_flutter.dart';

class FileTransferService {
  FileTransferService(this._connections);

  final ConnectionService _connections;
  final Map<String, TransferTask> _tasks = <String, TransferTask>{};
  final StreamController<TransferTask> _taskController =
      StreamController<TransferTask>.broadcast();

  List<TransferTask> get tasks => _tasks.values.toList();

  Stream<TransferTask> get taskStream => _taskController.stream;

  Future<TransferTask> sendFile(Device target, String filePath) async {
    final file = File(filePath);
    if (!await file.exists()) {
      throw TransferException('文件不存在: $filePath');
    }
    final size = await file.length();
    if (size > AppConstants.maxFileSize) {
      throw TransferException('文件超过最大限制 (${AppConstants.maxFileSize} bytes)');
    }

    final task = TransferTask(
      taskId: const Uuid().v4(),
      fileName: file.path.split(Platform.pathSeparator).last,
      fileSize: size,
      mimeType: 'application/octet-stream',
      targetDeviceId: target.deviceId,
      direction: TransferDirection.send,
      status: TransferStatus.pending,
      localPath: filePath,
    );
    _tasks[task.taskId] = task;
    _emit(task);

    // 使用独立文件传输端口建立连接
    var conn = _connections.get(target.deviceId);
    if (conn == null) {
      conn = await _connections.connect(target);
    }

    task.status = TransferStatus.transferring;
    _emit(task);

    final chunkSize = AppConstants.defaultChunkSize;
    final totalChunks = (size / chunkSize).ceil();
    final raf = await file.open();
    try {
      for (var i = 0; i < totalChunks; i++) {
        if (task.status == TransferStatus.canceled) break;
        await raf.setPosition(i * chunkSize);
        final bytes = await raf.read(chunkSize);
        task.transferredBytes += bytes.length;
        _emit(task);
        // 模拟写入远程（此处通过已建立的 connection 发送二进制数据）
        try {
          conn.socket.add(bytes);
          await conn.socket.flush();
        } catch (e) {
          task.status = TransferStatus.failed;
          task.errorMessage = e.toString();
          _emit(task);
          throw TransferException('传输中断', e);
        }
      }
      if (task.status != TransferStatus.canceled) {
        task.status = TransferStatus.done;
      }
      _emit(task);
      await DeviceRepository.instance.save(target);
    } finally {
      await raf.close();
    }
    return task;
  }

  Future<TransferTask> receiveFile({
    required Device fromDevice,
    required String fileName,
    required int fileSize,
    required String savePath,
    required Stream<List<int>> data,
  }) async {
    final task = TransferTask(
      taskId: const Uuid().v4(),
      fileName: fileName,
      fileSize: fileSize,
      mimeType: 'application/octet-stream',
      targetDeviceId: fromDevice.deviceId,
      direction: TransferDirection.receive,
      status: TransferStatus.transferring,
      localPath: savePath,
    );
    _tasks[task.taskId] = task;
    _emit(task);

    final file = File(savePath);
    final sink = file.openWrite();
    try {
      await for (final chunk in data) {
        if (task.status == TransferStatus.canceled) break;
        sink.add(chunk);
        task.transferredBytes += chunk.length;
        _emit(task);
      }
      await sink.flush();
      task.status = TransferStatus.done;
      _emit(task);
    } catch (e) {
      task.status = TransferStatus.failed;
      task.errorMessage = e.toString();
      _emit(task);
      rethrow;
    } finally {
      await sink.close();
    }
    return task;
  }

  Future<void> cancelTransfer(String taskId) async {
    final task = _tasks[taskId];
    if (task == null) return;
    task.status = TransferStatus.canceled;
    _emit(task);
  }

  void _emit(TransferTask task) {
    if (!_taskController.isClosed) _taskController.add(task);
  }

  Future<void> dispose() async {
    await _taskController.close();
  }
}

class TransferHistory {
  TransferHistory._();

  static final TransferHistory instance = TransferHistory._();

  Box<TransferTask>? _box;

  Future<Box<TransferTask>> get box async {
    return _box ??= await Hive.openBox<TransferTask>(AppConstants.hiveBoxTransfers);
  }

  Future<List<TransferTask>> all() async {
    final b = await box;
    return b.values.toList();
  }

  Future<void> save(TransferTask task) async {
    final b = await box;
    await b.add(task);
  }
}
