import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';

import '../../features/file_transfer/pages/file_transfer_page.dart';
import '../../features/remote_control/pages/remote_control_page.dart';
import '../pages/splash_page.dart';
import '../../features/device_management/pages/home_page.dart';
import '../../features/device_discovery/pages/discovery_page.dart';
import '../../features/device_management/pages/device_detail_page.dart';

final routerProvider = Provider<GoRouter>((ref) {
  return AppRouter.router;
});

class AppRouter {
  AppRouter._();

  static final router = GoRouter(
    initialLocation: '/splash',
    routes: [
      GoRoute(
        path: '/splash',
        builder: (_, __) => const SplashPage(),
      ),
      GoRoute(
        path: '/',
        builder: (_, __) => const HomePage(),
      ),
      GoRoute(
        path: '/discovery',
        builder: (_, __) => const DiscoveryPage(),
      ),
      GoRoute(
        path: '/device/:deviceId',
        builder: (context, state) {
          final deviceId = state.pathParameters['deviceId'] ?? '';
          return DeviceDetailPage(deviceId: deviceId);
        },
      ),
      GoRoute(
        path: '/control/:deviceId',
        builder: (context, state) {
          final deviceId = state.pathParameters['deviceId'] ?? '';
          return RemoteControlPage(deviceId: deviceId);
        },
      ),
      GoRoute(
        path: '/transfer/:deviceId',
        builder: (context, state) {
          final deviceId = state.pathParameters['deviceId'] ?? '';
          return FileTransferPage(deviceId: deviceId);
        },
      ),
    ],
  );
}
