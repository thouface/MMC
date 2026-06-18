import Foundation
import UIKit
import ReplayKit

/// Protocol for screen capture delegate
@objc public protocol ScreenCaptureServiceDelegate: AnyObject {
    func screenCaptureDidStart()
    func screenCaptureDidStop()
    func screenCaptureDidFail(_ error: Error)
    func screenCaptureDidReceiveFrame(_ sampleBuffer: CMSampleBuffer)
}

/// Screen capture service using ReplayKit for iOS
@objc public class ScreenCaptureService: NSObject {
    
    /// Delegate for screen capture events
    @objc public weak var delegate: ScreenCaptureServiceDelegate?
    
    /// Whether capture is running
    @objc public private(set) var isCapturing: Bool = false
    
    /// ReplayKit screen recorder
    private let recorder = RPScreenRecorder.shared()
    
    /// Video encoder
    private var videoEncoder: VideoEncoder?
    
    /// Capture configuration
    @objc public var config = ScreenCaptureConfig()
    
    private override init() {
        super.init()
        setupNotifications()
    }
    
    deinit {
        NotificationCenter.default.removeObserver(self)
    }
    
    // MARK: - Public Methods
    
    /// Check if screen recording is available
    @objc public func isAvailable() -> Bool {
        return recorder.isAvailable
    }
    
    /// Start screen capture
    @objc public func startCapture() throws {
        guard !isCapturing else { return }
        guard recorder.isAvailable else {
            throw ScreenCaptureError.notAvailable
        }
        
        // Initialize video encoder
        videoEncoder = VideoEncoder(
            width: config.width,
            height: config.height,
            bitrate: config.bitrate
        )
        
        // Start ReplayKit recording
        recorder.startCapture { [weak self] sampleBuffer, bufferType, error in
            guard let self = self else { return }
            
            if let error = error {
                DispatchQueue.main.async {
                    self.delegate?.screenCaptureDidFail(error)
                }
                return
            }
            
            guard let sampleBuffer = sampleBuffer else { return }
            
            // Process the frame
            self.processFrame(sampleBuffer, bufferType: bufferType)
            
            DispatchQueue.main.async {
                self.delegate?.screenCaptureDidReceiveFrame(sampleBuffer)
            }
        } completionHandler: { [weak self] error in
            if let error = error {
                DispatchQueue.main.async {
                    self?.delegate?.screenCaptureDidFail(error)
                }
                return
            }
            
            self?.isCapturing = true
            DispatchQueue.main.async {
                self?.delegate?.screenCaptureDidStart()
            }
            
            MmcCoreBridge.log("Screen capture started")
        }
    }
    
    /// Stop screen capture
    @objc public func stopCapture() {
        guard isCapturing else { return }
        
        recorder.stopCapture { [weak self] error in
            self?.isCapturing = false
            self?.videoEncoder = nil
            
            DispatchQueue.main.async {
                self?.delegate?.screenCaptureDidStop()
            }
            
            MmcCoreBridge.log("Screen capture stopped")
        }
    }
    
    // MARK: - Private Methods
    
    private func setupNotifications() {
        // Listen for app lifecycle events
        NotificationCenter.default.addObserver(
            self,
            selector: #selector(appDidBecomeActive),
            name: UIApplication.didBecomeActiveNotification,
            object: nil
        )
        
        NotificationCenter.default.addObserver(
            self,
            selector: #selector(appWillResignActive),
            name: UIApplication.willResignActiveNotification,
            object: nil
        )
    }
    
    @objc private func appDidBecomeActive() {
        // Resume capture if it was running
        if !isCapturing && config.autoResume {
            try? startCapture()
        }
    }
    
    @objc private func appWillResignActive() {
        // Pause capture
        if isCapturing && config.autoPause {
            stopCapture()
        }
    }
    
    private func processFrame(_ sampleBuffer: CMSampleBuffer, bufferType: RPSampleBufferType) {
        guard bufferType == .video else { return }
        
        // Encode frame for transmission
        if let encoder = videoEncoder {
            encoder.encode(sampleBuffer)
        }
    }
}

/// Screen capture configuration
@objc public class ScreenCaptureConfig: NSObject {
    /// Video width
    @objc public var width: Int32 = 1920
    
    /// Video height
    @objc public var height: Int32 = 1080
    
    /// Video bitrate in bps
    @objc public var bitrate: Int32 = 4_000_000
    
    /// Frame rate
    @objc public var frameRate: Int32 = 30
    
    /// Auto-pause when app goes to background
    @objc public var autoPause: Bool = true
    
    /// Auto-resume when app becomes active
    @objc public var autoResume: Bool = false
}

/// Screen capture errors
@objc public enum ScreenCaptureError: Int, Error {
    case notAvailable
    case not permitted
    case encodingFailed
    case unknown
}

/// Simple video encoder
private class VideoEncoder {
    private let width: Int32
    private let height: Int32
    private let bitrate: Int32
    
    init(width: Int32, height: Int32, bitrate: Int32) {
        self.width = width
        self.height = height
        self.bitrate = bitrate
    }
    
    func encode(_ sampleBuffer: CMSampleBuffer) {
        // In real implementation, this would use VideoToolbox for hardware encoding
        // For now, this is a stub that logs the frame
        MmcCoreBridge.onVideoFrame(Int(width), height: Int(height))
    }
}
