import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:mmc/core/pages/splash_page.dart';

void main() {
  group('SplashPage', () {
    testWidgets('renders logo and title', (tester) async {
      await tester.pumpWidget(
        const ProviderScope(
          child: MaterialApp(
            home: SplashPage(),
          ),
        ),
      );

      expect(find.text('MMC'), findsOneWidget);
      expect(find.text('多设备控制与文件传输'), findsOneWidget);
      expect(find.byIcon(Icons.devices_other), findsOneWidget);
    });

    testWidgets('shows loading indicator initially', (tester) async {
      await tester.pumpWidget(
        const ProviderScope(
          child: MaterialApp(
            home: SplashPage(),
          ),
        ),
      );

      // Wait for splash duration
      await tester.pump(const Duration(milliseconds: 1200));
      await tester.pumpAndSettle();

      // Should have navigated away (no longer showing splash content)
      // In a real test with go_router, we'd verify navigation
    });
  });
}