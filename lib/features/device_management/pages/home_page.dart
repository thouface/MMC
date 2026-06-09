import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';

import '../../device_discovery/pages/discovery_page.dart';
import '../providers/device_list_provider.dart';
import '../../../models/device_type_extension.dart';

class HomePage extends ConsumerWidget {
  const HomePage({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final devices = ref.watch(deviceListProvider);
    final online = devices.where((d) => d.status == DeviceStatus.online).toList();
    final offline = devices.where((d) => d.status != DeviceStatus.online).toList();

    return Scaffold(
      appBar: AppBar(
        title: const Text('设备列表'),
        actions: [
          IconButton(
            icon: const Icon(Icons.refresh),
            onPressed: () => ref.read(deviceListProvider.notifier).refresh(),
          ),
        ],
      ),
      body: devices.isEmpty
          ? const _EmptyState()
          : ListView(
              padding: const EdgeInsets.all(16),
              children: [
                if (online.isNotEmpty) ...[
                  Text('在线 (${online.length})',
                      style: Theme.of(context).textTheme.titleSmall),
                const SizedBox(height: 8),
                ...online.map((d) => _DeviceTile(device: d)),
              ],
              if (offline.isNotEmpty) ...[
                const SizedBox(height: 16),
                Text('离线 (${offline.length})',
                    style: Theme.of(context).textTheme.titleSmall),
                const SizedBox(height: 8),
                ...offline.map((d) => _DeviceTile(device: d)),
              ],
            ],
          ),
      floatingActionButton: FloatingActionButton.extended(
        icon: const Icon(Icons.add),
        label: const Text('添加设备'),
        onPressed: () => context.go('/discovery'),
      ),
    );
  }
}

class _EmptyState extends StatelessWidget {
  const _EmptyState();

  @override
  Widget build(BuildContext context) {
    return Center(
      child: Column(
        mainAxisAlignment: MainAxisAlignment.center,
        children: [
          Icon(
            Icons.devices_other,
            size: 80,
            color: Theme.of(context).colorScheme.primary.withValues(alpha: 0.5),
          ),
          const SizedBox(height: 16),
          Text('暂无配对的设备', style: Theme.of(context).textTheme.titleMedium),
          const SizedBox(height: 8),
          Text('点击右下角按钮开始发现附近设备',
              style: Theme.of(context).textTheme.bodyMedium),
        ],
      ),
    );
  }
}

class _DeviceTile extends ConsumerWidget {
  const _DeviceTile({required this.device});

  final Device device;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    return Card(
      child: ListTile(
    leading: CircleAvatar(child: Icon(device.type.icon),
    title: Text(device.name),
    subtitle: Text('${device.type.displayName} · ${device.os}'),
    trailing: Icon(
      device.status == DeviceStatus.online ? Icons.circle : Icons.circle_outlined,
      color: device.status == DeviceStatus.online ? Colors.green : Colors.grey,
      size: 12,
    ),
    onTap: () => context.go('/device/${device.deviceId}'),
    onLongPress: () => showDialog(
      context: context,
      builder: (_) => AlertDialog(
        title: Text(device.name),
        actions: [
          TextButton(
            onPressed: () {
              Navigator.of(context).pop();
              _showRenameDialog(context, ref);
            },
            child: const Text('重命名'),
          ),
          TextButton(
            onPressed: () {
              ref.read(deviceListProvider.notifier).remove(device.deviceId);
              Navigator.of(context).pop();
            },
            child: const Text('删除'),
          ),
          TextButton(
            onPressed: () => Navigator.of(context).pop(),
            child: const Text('关闭'),
          ),
        ],
      ),
    ),
  );
  }

  void _showRenameDialog(BuildContext context, WidgetRef ref) {
    final controller = TextEditingController(text: device.name);
    showDialog(
      context: context,
      builder: (_) => AlertDialog(
        title: const Text('重命名设备'),
        content: TextField(
          controller: controller,
          decoration: const InputDecoration(hintText: '输入新名称'),
        ),
        actions: [
          TextButton(
            onPressed: () => Navigator.of(context).pop(),
            child: const Text('取消'),
          ),
          TextButton(
            onPressed: () {
              ref.read(deviceListProvider.notifier).rename(device.deviceId, controller.text);
              Navigator.of(context).pop();
            },
            child: const Text('保存'),
          ),
        ],
      ),
    );
  }
}
