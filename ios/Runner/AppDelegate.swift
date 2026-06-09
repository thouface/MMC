import UIKit
import Flutter

@UIApplicationMain
@objc class AppDelegate: FlutterAppDelegate {
  private let CHANNEL = "com.example.mmc/platform"
  private var serviceBrowser: NetServiceBrowser?
  private var discoveredServices: [NetService] = []

  override func application(
    _ application: UIApplication,
    didFinishLaunchingWithOptions launchOptions: [UIApplication.LaunchOptionsKey: Any]?
  ) -> Bool {
    guard let controller = window?.rootViewController as? FlutterViewController else {
      return super.application(application, didFinishLaunchingWithOptions: launchOptions)
    }
    let channel = FlutterMethodChannel(name: CHANNEL, binaryMessenger: controller.binaryMessenger)
    channel.setMethodCallHandler { [weak self] call, result in
      switch call.method {
      case "startDiscovery":
        self?.startDiscovery(serviceType: call.arguments as? String ?? "_thefool._tcp", result: result)
      case "stopDiscovery":
        self?.stopDiscovery(result: result)
      case "getDeviceInfo":
        result.success([
          "model": UIDevice.current.model,
          "os": "ios-\(UIDevice.current.systemVersion)"
        ])
      default:
        result(FlutterMethodNotImplemented)
      }
    }
    return super.application(application, didFinishLaunchingWithOptions: launchOptions)
  }

  private func startDiscovery(serviceType: String, result: @escaping FlutterResult) {
    serviceBrowser = NetServiceBrowser()
    serviceBrowser?.delegate = self
    serviceBrowser?.searchForServices(ofType: serviceType, inDomain: "local.")
    result(nil)
  }

  private func stopDiscovery(_ result: @escaping FlutterResult) {
    serviceBrowser?.stop()
    result(nil)
  }
}

extension AppDelegate: NetServiceBrowserDelegate, NetServiceDelegate {
  func netServiceBrowser(_ browser: NetServiceBrowser, didFind service: NetService, moreComing: Bool) {
    service.delegate = self
    service.resolve(withTimeout: 5)
    discoveredServices.append(service)
  }

  func netServiceDidResolveAddress(_ sender: NetService) {
    guard let addresses = sender.addresses, let data = addresses.first else { return }
    let hostname = data.withUnsafeBytes { raw -> String? in
      guard let baseAddress = raw.baseAddress else { return nil }
      let sa = baseAddress.assumingMemoryBound(to: sockaddr_in.self)
      let buffer = UnsafeMutablePointer<Int8>.allocate(capacity: Int(INET_ADDRSTRLEN))
      inet_ntop(AF_INET, &sa.pointee.sin_addr, buffer, socklen_t(INET_ADDRSTRLEN))
      return String(cString: buffer)
    }
    let info: [String: Any] = [
      "name": sender.name,
      "type": sender.type,
      "port": sender.port,
      "host": hostname ?? ""
    ]
    if let controller = window?.rootViewController as? FlutterViewController {
      FlutterMethodChannel(name: CHANNEL, binaryMessenger: controller.binaryMessenger)
        .invokeMethod("onDeviceFound", arguments: info)
    }
  }
}
