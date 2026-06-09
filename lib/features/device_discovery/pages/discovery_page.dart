import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';

import '../providers/discovery_provider.dart';
import '../../../models/device_type_extension.dart';

class DiscoveryPage extends ConsumerStatefulWidget {
  const DiscoveryPage({super.key});

  @override
  ConsumerState<DiscoveryPage> createState() => _DiscoveryPageState();
}

class _DiscoveryPageState extends ConsumerState<DiscoveryPage> {
  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addPostFrameCallback((_) {
      ref.read(discoveryProvider.notifier).start();
    });
  }

  @override
  Widget build(BuildContext context) {
    final state = ref.watch(discoveryProvider);
    return Scaffold(
      appBar: AppBar(title: const Text('设备发现'),
      body: state.when(
        loading: () => const Center(
          child: Column(
            mainAxisAlignment: MainAxisAlignment.center,
            children: [
              const CircularProgressIndicator(),
              const SizedBox(height: 16),
              Text('正在扫描局域网...', style: Theme.of(context).textTheme.bodyLarge),
            ],
          ),
        ),
        error: (e, _) => Center(child: Text('扫描失败: $e')),
        data: (devices) => devices.isEmpty
            ? const Center(child: Text('未发现设备'))
            : ListView.builder(
                padding: const EdgeInsets.all(16),
                itemCount: devices.length,
                itemBuilder: (_, i) {
                  final device = devices[i];
                  return Card(
                    child: ListTile(
                      leading: CircleAvatar(child: Icon(device.type.icon),
                      title: Text(device.name),
                      subtitle: Text('${device.ip ?? ''} · ${device.os}'),
                      trailing: ElevatedButton(
                        onPressed: () async {
                          await ref.read(discoveryProvider.notifier).pair(device);
                          if (context.mounted) context.go('/');
                        },
                        child: const Text('配对'),
                      ),
                    ),
                  );
                },
              ),
      ),
      floatingActionButton: FloatingActionButton.extended(
        icon: const Icon(Icons.edit),
        label: const Text('手动配对'),
        onPressed: () => _showManualDialog(context, ref),
      ),
    );
  }

  void _showManualDialog(BuildContext context, WidgetRef ref) {
    final ipController = TextEditingController();
    final portController = TextEditingController(text: '54321');
    final nameController = TextEditingController(text: '设备');
    showDialog(
      context: context,
      builder: (_) => AlertDialog(
        title: const Text('手动配对'),
        content: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            TextField(
              controller: ipController,
              decoration: const InputDecoration(hintText: 'IP 地址'),
            ),
            TextField(
              controller: portController,
              decoration: const InputDecoration(hintText: '端口'),
              keyboardType: TextInputType.number,
            ),
            TextField(
              controller: nameController,
              decoration: const InputDecoration(hintText: '名称'),
            ),
          ],
        ),
        actions: [
          TextButton(
            onPressed: () => Navigator.of(context).pop(),
            child: const Text('取消'),
          ),
          ElevatedButton(
            onPressed: () async {
              final ip = ipController.text.trim();
              final port = int.tryParse(portController.text.trim()) ?? 54321;
              final name = nameController.text.trim();
              if (ip.isEmpty) return;
              await ref.read(discoveryProvider.notifier).manualPair(ip, port, name);
              if (context.mounted) {
                Navigator.of(context).pop();
                context.go('/');
              }
            },
            child: const Text('配对'),
          ),
        ],
      ),
    );
  }
}
