import Foundation

/// Protocol for clipboard monitoring delegate
@objc public protocol ClipboardMonitorDelegate: AnyObject {
    func clipboardDidChange(_ content: String)
    func clipboardDidChangeImage(_ imageData: Data)
}

/// Clipboard monitoring service for iOS
@objc public class ClipboardMonitor: NSObject {
    
    /// Delegate for clipboard changes
    @objc public weak var delegate: ClipboardMonitorDelegate?
    
    /// Whether monitoring is active
    @objc public private(set) var isMonitoring: Bool = false
    
    /// Timer for polling clipboard changes
    private var monitorTimer: Timer?
    
    /// Last known clipboard content hash
    private var lastContentHash: Int = 0
    
    /// Start monitoring clipboard changes
    @objc public func start() throws {
        guard !isMonitoring else { return }
        
        isMonitoring = true
        lastContentHash = getCurrentContentHash()
        
        // Poll clipboard every 0.5 seconds
        monitorTimer = Timer.scheduledTimer(withTimeInterval: 0.5, repeats: true) { [weak self] _ in
            self?.checkClipboard()
        }
        
        MmcCoreBridge.log("Clipboard monitoring started")
    }
    
    /// Stop monitoring clipboard changes
    @objc public func stop() {
        monitorTimer?.invalidate()
        monitorTimer = nil
        isMonitoring = false
        
        MmcCoreBridge.log("Clipboard monitoring stopped")
    }
    
    /// Get current clipboard text
    @objc public func getClipboardText() -> String? {
        return UIPasteboard.general.string
    }
    
    /// Get current clipboard image as PNG data
    @objc public func getClipboardImage() -> Data? {
        return UIPasteboard.general.png
    }
    
    /// Set clipboard text
    @objc public func setClipboardText(_ text: String) {
        UIPasteboard.general.string = text
        lastContentHash = getCurrentContentHash()
    }
    
    /// Set clipboard image
    @objc public func setClipboardImage(_ imageData: Data) {
        if let image = UIImage(data: imageData) {
            UIPasteboard.general.image = image
            lastContentHash = getCurrentContentHash()
        }
    }
    
    // MARK: - Private Methods
    
    private func checkClipboard() {
        let currentHash = getCurrentContentHash()
        
        if currentHash != lastContentHash {
            lastContentHash = currentHash
            
            // Check for text change
            if let text = UIPasteboard.general.string {
                delegate?.clipboardDidChange(text)
                MmcCoreBridge.onClipboardChange(text)
            }
            // Check for image change
            else if let imageData = UIPasteboard.general.png {
                delegate?.clipboardDidChangeImage(imageData)
            }
        }
    }
    
    private func getCurrentContentHash() -> Int {
        if let text = UIPasteboard.general.string {
            return text.hashValue
        } else if UIPasteboard.general.hasImages {
            return UIPasteboard.general.image?.pngData()?.hashValue ?? 0
        }
        return 0
    }
}
