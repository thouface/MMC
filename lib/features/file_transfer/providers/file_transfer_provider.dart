import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:file_picker/file_picker.dart';

import '../../models/device.dart';
import '../../models/transfer_task.dart';
import '../../services/connection_service.dart';
import '../../services/file_transfer_service.dart';

class FileTransferNotifier extends Notifier<List<TransferTask>> {
  final _conn = ConnectionService();
  late final FileTransferService _transfer = FileTransferService(_conn);

  @override
  List<TransferTask> build() {
    ref.onDispose(() => _transfer.dispose());
    _transfer.taskStream.listen((task) {
      state = [...state.where((t) => t.taskId != task.taskId), task];
    });
    return [];
  }

  Future<TransferTask?> sendFile(Device target) async {
    final result = await FilePicker.platform.pickFiles();
    if (result == null || result.files.first.path == null) return null;
    final task = await _transfer.sendFile(target, result.files.first.path!);
    state = [...state, task];
    return task;
  }

  Future<void> cancel(String taskId) async {
    await _transfer.cancelTransfer(taskId);
  }

  void reset() {
    state = [];
  }
}

final fileTransferProvider =
    NotifierProvider<FileTransferNotifier, List<TransferTask>>(
  FileTransferNotifier.new,
);
