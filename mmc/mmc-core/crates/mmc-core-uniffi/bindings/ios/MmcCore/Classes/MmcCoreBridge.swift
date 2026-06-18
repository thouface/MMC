import Foundation

/// Bridge between Swift and Rust native library
@objc public class MmcCoreBridge: NSObject {
    
    /// Initialize the native library
    @objc public static func initialize() {
        // Call Rust FFI initialization
        mmc_core_initialize()
        log("MMC Core initialized")
    }
    
    /// Log a message from native code
    @objc public static func log(_ message: String) {
        NSLog("[MMC] %@", message)
    }
    
    /// Handle clipboard change from Swift
    @objc public static func onClipboardChange(_ text: String) {
        // Notify Rust side of clipboard change
        let textCString = text.cString(using: .utf8)!
        mmc_core_on_clipboard_change(textCString)
    }
    
    /// Handle video frame from Swift
    @objc public static func onVideoFrame(_ width: Int, height: Int) {
        // Notify Rust side of video frame
        mmc_core_on_video_frame(Int32(width), Int32(height))
    }
    
    /// Get device ID
    @objc public static func getDeviceId() -> String {
        var buffer: UnsafeMutablePointer<CChar>?
        var length: Int = 0
        
        if mmc_core_get_device_id(&buffer, &length) == 0, let buffer = buffer {
            let result = String(cString: buffer)
            free(buffer)
            return result
        }
        
        return UUID().uuidString
    }
}

// MARK: - FFI Declarations

@_silgen_name("mmc_core_initialize")
private func mmc_core_initialize()

@_silgen_name("mmc_core_on_clipboard_change")
private func mmc_core_on_clipboard_change(_ text: UnsafePointer<CChar>)

@_silgen_name("mmc_core_on_video_frame")
private func mmc_core_on_video_frame(_ width: Int32, _ height: Int32)

@_silgen_name("mmc_core_get_device_id")
private func mmc_core_get_device_id(
    _ buffer: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>?,
    _ length: UnsafeMutablePointer<Int>?
) -> Int32
