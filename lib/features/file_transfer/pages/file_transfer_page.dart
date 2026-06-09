import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';

import '../providers/file_transfer_provider.dart';
import '../../device_management/providers/device_list_provider.dart';

class FileTransferPage extends ConsumerWidget {
  const FileTransferPage({super.key, required this.deviceId});

  final String deviceId;

  String _formatBytes(int bytes) {
    if (bytes < 1024) return '$bytes B';
    if (bytes < 1024 * 1024) return '${(bytes / 1024).toStringAsFixed(1)} KB';
    if (bytes < 1024 * 1024 * 1024) return '${(bytes / 1024 / 1024).toStringAsFixed(1)} MB';
    return '${(bytes / 1024 / 1024 / 1024).toStringAsFixed(1)} GB';
  }

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final devices = ref.watch(deviceListProvider);
    final device = devices.firstWhere((d) => d.deviceId == deviceId);
    final tasks = ref.watch(fileTransferProvider);

    return Scaffold(
      appBar: AppBar(title: Text('向 ${device.name} 发送文件')),
      body: Column(
        children: [
          Padding(
            padding: const EdgeInsets.all(16),
            child: ElevatedButton.icon(
              onPressed: () async {
                await ref.read(fileTransferProvider.notifier).sendFile(device);
              },
              icon: const Icon(Icons.attach_file),
              label: const Text('选择文件并发送'),
              style: ElevatedButton.styleFrom(padding: const EdgeInsets.all(16)),
            ),
          ),
          const Divider(),
          Expanded(
            child: tasks.isEmpty
                ? const Center(child: Text('暂无传输任务'))
                : ListView.builder(
                    padding: const EdgeInsets.all(16),
                    itemCount: tasks.length,
                    itemBuilder: (_, i) {
                      final task = tasks[i];
                      return Card(
                        child: ListTile(
                          title: Text(task.fileName),
                          subtitle: Column(
                            crossAxisAlignment: CrossAxisAlignment.start,
                            children: [
                              Text(
                                '${_formatBytes(task.transferredBytes)} / ${_formatBytes(task.fileSize)} · ${task.status.name}',
                              ),
                              const SizedBox(height: 4),
                              LinearProgressIndicator(value: task.progress.clamp(0.0, 1.0)),
                            ],
                          ),
                          trailing: IconButton(
                            icon: const Icon(Icons.cancel),
                            onPressed: () => ref.read(fileTransferProvider.notifier).cancel(task.taskId),
                          ),
                        ),
                      );
                    },
                  ),
          ),
        ],
      ),
    );
  }
}
