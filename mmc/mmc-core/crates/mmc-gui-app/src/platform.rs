//! Platform-specific functionality

use std::env;

#[derive(Debug, Clone)]
pub struct PlatformInfo {
    pub os: String,
    pub arch: String,
    pub hostname: String,
    pub device_id: String,
}

pub fn get_platform_info() -> PlatformInfo {
    let os = match env::consts::OS {
        "windows" => "Windows",
        "macos" => "macOS",
        "linux" => "Linux",
        "android" => "Android",
        "ios" => "iOS",
        other => other,
    };

    let arch = match env::consts::ARCH {
        "x86_64" => "x64",
        "aarch64" => "arm64",
        "x86" => "x86",
        "arm" => "arm",
        other => other,
    };

    let hostname = hostname::get()
        .map(|h| h.to_string_lossy().to_string())
        .unwrap_or_else(|_| "Unknown".to_string());

    // Generate device ID from hostname and random component
    use uuid::Uuid;
    let uuid = Uuid::new_v4();
    let device_id = format!("{}-{}", hostname.replace("-", "").replace("_", ""), &uuid.to_string()[..8]);

    PlatformInfo {
        os: os.to_string(),
        arch: arch.to_string(),
        hostname,
        device_id,
    }
}

#[cfg(windows)]
pub fn get_screen_size() -> (u32, u32) {
    use windows::Win32::Graphics::Gdi::GetDC;
    use windows::Win32::UI::WindowsAndMessaging::{GetSystemMetrics, SM_CXSCREEN, SM_CYSCREEN};
    use windows::Win32::Foundation::HWND;

    unsafe {
        let hdc = GetDC(HWND::default());
        let width = GetSystemMetrics(SM_CXSCREEN);
        let height = GetSystemMetrics(SM_CYSCREEN);
        (width as u32, height as u32)
    }
}

#[cfg(not(windows))]
pub fn get_screen_size() -> (u32, u32) {
    (1920, 1080)
}
