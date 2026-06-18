import Foundation

/// Main entry point for MMC Core functionality on iOS
@objc public class MmcCore: NSObject {
    
    /// Shared singleton instance
    @objc public static let shared = MmcCore()
    
    /// Current device ID
    @objc public private(set) var deviceId: String = ""
    
    /// Current device name
    @objc public private(set) var deviceName: String = ""
    
    /// Clipboard monitor instance
    @objc public let clipboardMonitor: ClipboardMonitor
    
    /// Screen capture service instance
    @objc public let screenCaptureService: ScreenCaptureService
    
    /// Transfer service instance
    @objc public let transferService: TransferService
    
    private override init() {
        // Get device info
        self.deviceId = UIDevice.current.identifierForVendor?.uuidString ?? UUID().uuidString
        self.deviceName = UIDevice.current.name
        
        // Initialize services
        self.clipboardMonitor = ClipboardMonitor()
        self.screenCaptureService = ScreenCaptureService()
        self.transferService = TransferService()
        
        super.init()
    }
    
    /// Initialize the core framework
    @objc public func initialize() throws {
        // Initialize native library
        MmcCoreBridge.initialize()
    }
    
    /// Get device information
    @objc public func getDeviceInfo() -> [String: Any] {
        return [
            "deviceId": deviceId,
            "deviceName": deviceName,
            "platform": "ios",
            "osVersion": UIDevice.current.systemVersion,
            "appVersion": Bundle.main.infoDictionary?["CFBundleShortVersionString"] as? String ?? "1.0"
        ]
    }
    
    /// Start clipboard monitoring
    @objc public func startClipboardMonitoring() throws {
        try clipboardMonitor.start()
    }
    
    /// Stop clipboard monitoring
    @objc public func stopClipboardMonitoring() {
        clipboardMonitor.stop()
    }
}
