import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:hive_flutter/hive_flutter.dart';

import 'core/router/app_router.dart';
import 'core/theme/app_theme.dart';
import 'models/device.dart';
import 'models/transfer_task.dart';

Future<void> main() async {
  WidgetsFlutterBinding.ensureInitialized();

  await Hive.initFlutter();
  Hive.registerAdapter(DeviceAdapter());
  Hive.registerAdapter(DeviceTypeAdapter());
  Hive.registerAdapter(DeviceStatusAdapter());
  Hive.registerAdapter(TransferTaskAdapter());
  Hive.registerAdapter(TransferDirectionAdapter());
  Hive.registerAdapter(TransferStatusAdapter());

  runApp(const ProviderScope(child: MMCApp()));
}

class MMCApp extends StatelessWidget {
  const MMCApp({super.key});

  @override
  Widget build(BuildContext context) {
    return MaterialApp.router(
      title: 'MMC',
      debugShowCheckedModeBanner: false,
      theme: AppTheme.light,
      darkTheme: AppTheme.dark,
      routerConfig: AppRouter.router,
    );
  }
}
