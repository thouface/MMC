import Foundation

/// Protocol for transfer service delegate
@objc public protocol TransferServiceDelegate: AnyObject {
    func transferDidStart(_ taskId: String)
    func transferDidProgress(_ taskId: String, progress: Float)
    func transferDidComplete(_ taskId: String)
    func transferDidFail(_ taskId: String, error: Error)
}

/// File transfer service for iOS
@objc public class TransferService: NSObject {
    
    /// Delegate for transfer events
    @objc public weak var delegate: TransferServiceDelegate?
    
    /// Active transfers
    private var activeTransfers: [String: TransferTask] = [:]
    
    private override init() {
        super.init()
    }
    
    // MARK: - Public Methods
    
    /// Send a file to a device
    @objc public func sendFile(
        _ fileURL: URL,
        toDevice deviceId: String,
        completion: @escaping (Result<String, Error>) -> Void
    ) {
        // Validate file exists
        guard FileManager.default.fileExists(atPath: fileURL.path) else {
            completion(.failure(TransferError.fileNotFound))
            return
        }
        
        // Create transfer task
        let taskId = UUID().uuidString
        let task = TransferTask(
            id: taskId,
            fileURL: fileURL,
            targetDevice: deviceId,
            state: .preparing
        )
        
        activeTransfers[taskId] = task
        delegate?.transferDidStart(taskId)
        
        // Start transfer in background
        DispatchQueue.global(qos: .userInitiated).async { [weak self] in
            self?.performTransfer(task) { result in
                DispatchQueue.main.async {
                    switch result {
                    case .success:
                        self?.delegate?.transferDidComplete(taskId)
                    case .failure(let error):
                        self?.delegate?.transferDidFail(taskId, error: error)
                    }
                    self?.activeTransfers.removeValue(forKey: taskId)
                }
            }
        }
        
        completion(.success(taskId))
    }
    
    /// Cancel an active transfer
    @objc public func cancelTransfer(_ taskId: String) {
        if let task = activeTransfers[taskId] {
            task.state = .cancelled
            activeTransfers.removeValue(forKey: taskId)
        }
    }
    
    /// Get list of active transfers
    @objc public func getActiveTransfers() -> [[String: Any]] {
        return activeTransfers.values.map { task in
            [
                "id": task.id,
                "fileName": task.fileURL.lastPathComponent,
                "state": task.state.rawValue,
                "progress": task.progress
            ]
        }
    }
    
    // MARK: - Private Methods
    
    private func performTransfer(
        _ task: TransferTask,
        completion: @escaping (Result<Void, Error>) -> Void
    ) {
        task.state = .transferring
        
        // Get file data
        guard let fileData = try? Data(contentsOf: task.fileURL) else {
            completion(.failure(TransferError.fileNotFound))
            return
        }
        
        let totalBytes = fileData.count
        let chunkSize = 64 * 1024 // 64KB chunks
        var offset = 0
        
        // Simulate chunked transfer with progress updates
        while offset < totalBytes && task.state != .cancelled {
            let end = min(offset + chunkSize, totalBytes)
            let chunk = fileData[offset..<end]
            
            // Simulate network delay
            Thread.sleep(forTimeInterval: 0.01)
            
            // Update progress
            task.progress = Float(end) / Float(totalBytes)
            
            DispatchQueue.main.async { [weak self] in
                self?.delegate?.transferDidProgress(task.id, progress: task.progress)
            }
            
            offset = end
        }
        
        if task.state == .cancelled {
            completion(.failure(TransferError.cancelled))
        } else {
            task.state = .completed
            completion(.success(()))
        }
    }
}

/// Transfer task state
@objc public enum TransferState: Int {
    case preparing
    case transferring
    case completed
    case failed
    case cancelled
}

/// Transfer task
private class TransferTask {
    let id: String
    let fileURL: URL
    let targetDevice: String
    var state: TransferState
    var progress: Float = 0.0
    
    init(id: String, fileURL: URL, targetDevice: String, state: TransferState) {
        self.id = id
        self.fileURL = fileURL
        self.targetDevice = targetDevice
        self.state = state
    }
}

/// Transfer errors
@objc public enum TransferError: Int, Error {
    case fileNotFound
    case connectionFailed
    case cancelled
    case unknown
}
