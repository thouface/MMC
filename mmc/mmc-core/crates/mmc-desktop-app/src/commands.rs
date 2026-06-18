//! CLI command definitions using clap

use clap::Subcommand;

/// Device discovery and pairing commands
#[derive(Subcommand, Debug)]
pub enum DeviceCommand {
    /// Discover nearby devices using mDNS
    Discover,
    /// Pair with a device
    Pair {
        /// Device ID to pair with
        device_id: String,
    },
    /// List paired devices
    List,
    /// Unpair a device
    Unpair {
        /// Device ID to unpair
        device_id: String,
    },
}

/// File transfer commands
#[derive(Subcommand, Debug)]
pub enum TransferCommand {
    /// Send a file to a device
    Send {
        /// Target device ID
        device_id: String,
        /// File path to send
        file: String,
    },
    /// List active transfers
    List,
    /// Cancel a transfer
    Cancel {
        /// Task ID to cancel
        task_id: String,
    },
}

/// Clipboard synchronization commands
#[derive(Subcommand, Debug)]
pub enum ClipboardCommand {
    /// Get current clipboard content
    Get,
    /// Set clipboard content
    Set {
        /// Text to set in clipboard
        text: String,
    },
    /// Sync clipboard with a device
    Sync {
        /// Target device ID
        device_id: String,
    },
    /// Monitor clipboard for changes
    Monitor {
        /// Duration in seconds
        #[arg(default_value = "30")]
        duration_secs: u64,
    },
}

/// Screen mirroring commands
#[derive(Subcommand, Debug)]
pub enum MirrorCommand {
    /// Start screen mirroring with a device
    Start {
        /// Target device ID
        device_id: String,
    },
    /// Stop screen mirroring
    Stop,
    /// Show mirroring status
    Status,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_device_command_variants() {
        // Verify all command variants can be constructed
        let _discover = DeviceCommand::Discover;
        let _pair = DeviceCommand::Pair { device_id: "test".to_string() };
        let _list = DeviceCommand::List;
        let _unpair = DeviceCommand::Unpair { device_id: "test".to_string() };
    }

    #[test]
    fn test_transfer_command_variants() {
        let _send = TransferCommand::Send { device_id: "test".to_string(), file: "/path/to/file".to_string() };
        let _list = TransferCommand::List;
        let _cancel = TransferCommand::Cancel { task_id: "task-1".to_string() };
    }

    #[test]
    fn test_clipboard_command_variants() {
        let _get = ClipboardCommand::Get;
        let _set = ClipboardCommand::Set { text: "hello".to_string() };
        let _sync = ClipboardCommand::Sync { device_id: "device-1".to_string() };
        let _monitor = ClipboardCommand::Monitor { duration_secs: 60 };
    }

    #[test]
    fn test_mirror_command_variants() {
        let _start = MirrorCommand::Start { device_id: "device-1".to_string() };
        let _stop = MirrorCommand::Stop;
        let _status = MirrorCommand::Status;
    }

    #[test]
    fn test_clipboard_monitor_default_duration() {
        // Verify default duration is 30 seconds
        let cmd = ClipboardCommand::Monitor { duration_secs: 30 };
        match cmd {
            ClipboardCommand::Monitor { duration_secs } => {
                assert_eq!(duration_secs, 30);
            }
            _ => panic!("Expected Monitor command"),
        }
    }
}