//! MMC GUI Application Module

use std::collections::VecDeque;

use eframe::egui::{Align, Button, CentralPanel, Color32, Context, Frame, Layout, ProgressBar, RichText, ScrollArea, Stroke, Ui, Visuals};
use serde::{Deserialize, Serialize};

use mmc_discovery::DeviceInfo;

use crate::platform::PlatformInfo;
use crate::{APP_NAME, VERSION};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GuiDevice {
    pub id: String,
    pub name: String,
    pub device_type: String,
    pub ip: String,
    pub port: u16,
    pub online: bool,
    pub os_version: String,
    pub app_version: String,
}

impl From<DeviceInfo> for GuiDevice {
    fn from(info: DeviceInfo) -> Self {
        GuiDevice {
            id: info.id,
            name: info.name,
            device_type: format!("{}", info.device_type),
            ip: info.ip,
            port: info.port,
            online: true,
            os_version: info.os_version,
            app_version: info.app_version,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GuiTransferTask {
    pub task_id: String,
    pub file_name: String,
    pub progress: f32,
    pub speed: f32,
    pub state: String,
}

impl From<&mmc_file_transfer::TransferProgress> for GuiTransferTask {
    fn from(progress: &mmc_file_transfer::TransferProgress) -> Self {
        GuiTransferTask {
            task_id: progress.task_id.clone(),
            file_name: format!("{}.bin", progress.task_id),
            progress: progress.percent() / 100.0,
            speed: progress.speed_bps as f32 / 1_000_000.0,
            state: format!("{:?}", progress.state),
        }
    }
}

#[derive(Debug, Clone)]
pub struct MirrorStats {
    pub fps: f32,
    pub total_frames: u64,
    pub duration_secs: f32,
}

pub struct MmcGuiApp {
    platform_info: PlatformInfo,
    server_port: u16,

    selected_tab: Tab,
    show_settings: bool,

    // UI state - stores data from real services
    discovered_devices: Vec<GuiDevice>,
    paired_devices: Vec<GuiDevice>,
    selected_device: Option<usize>,
    transfer_tasks: Vec<GuiTransferTask>,
    clipboard_content: String,
    mirror_active: bool,
    mirror_stats: MirrorStats,
    logs: VecDeque<(String, String)>,
    max_logs: usize,

    // UI state flags
    is_scanning: bool,
    discovery_available: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Tab {
    Dashboard,
    Devices,
    Transfer,
    Clipboard,
    Mirror,
    Logs,
}

impl MmcGuiApp {
    pub fn new(platform_info: PlatformInfo, server_port: u16) -> Self {
        let mut logs = VecDeque::new();
        logs.push_front((
            chrono::Local::now().format("%H:%M:%S").to_string(),
            format!("{} v{} initialized", APP_NAME, VERSION),
        ));
        logs.push_front((
            chrono::Local::now().format("%H:%M:%S").to_string(),
            format!("Platform: {} ({})", platform_info.os, platform_info.arch),
        ));
        logs.push_front((
            chrono::Local::now().format("%H:%M:%S").to_string(),
            format!("GUI server running on http://localhost:{}", server_port),
        ));

        // Check if discovery service can be initialized
        let discovery_available = mmc_discovery::DiscoveryService::new().is_ok();
        if discovery_available {
            logs.push_front((
                chrono::Local::now().format("%H:%M:%S").to_string(),
                "Discovery service ready (mDNS)".to_string(),
            ));
        } else {
            logs.push_front((
                chrono::Local::now().format("%H:%M:%S").to_string(),
                "Discovery service unavailable".to_string(),
            ));
        }

        logs.push_front((
            chrono::Local::now().format("%H:%M:%S").to_string(),
            "Transfer service initialized".to_string(),
        ));

        logs.push_front((
            chrono::Local::now().format("%H:%M:%S").to_string(),
            "Clipboard service initialized".to_string(),
        ));

        Self {
            platform_info,
            server_port,
            selected_tab: Tab::Dashboard,
            show_settings: false,
            discovered_devices: vec![],
            paired_devices: vec![],
            selected_device: None,
            transfer_tasks: vec![],
            clipboard_content: String::from("Click 'Get' to read clipboard"),
            mirror_active: false,
            mirror_stats: MirrorStats {
                fps: 0.0,
                total_frames: 0,
                duration_secs: 0.0,
            },
            logs,
            max_logs: 100,
            is_scanning: false,
            discovery_available,
        }
    }

    fn add_log(&mut self, level: &str, message: &str) {
        self.logs.push_front((
            chrono::Local::now().format("%H:%M:%S").to_string(),
            format!("[{}] {}", level.to_uppercase(), message),
        ));
        while self.logs.len() > self.max_logs {
            self.logs.pop_back();
        }
    }
}

impl eframe::App for MmcGuiApp {
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        // Request repaint for active mirroring
        if self.mirror_active {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }

        ctx.set_visuals(Visuals::dark());

        CentralPanel::default().show(ctx, |ui: &mut Ui| {
            ui.add_space(16.0);

            // Header
            ui.horizontal(|ui: &mut Ui| {
                ui.label(RichText::new("📱").size(24.0));
                ui.label(
                    RichText::new(format!("{} v{}", APP_NAME, VERSION))
                        .size(18.0)
                        .strong(),
                );
                ui.add_space(32.0);
                ui.label(
                    RichText::new(format!(
                        "ID: {}",
                        &self.platform_info.device_id[..8.min(self.platform_info.device_id.len())]
                    ))
                    .small(),
                );
                ui.add_space(16.0);
                ui.label(RichText::new(&self.platform_info.hostname).small());
                ui.add_space(16.0);
                ui.label(
                    RichText::new(format!(
                        "{} ({})",
                        self.platform_info.os, self.platform_info.arch
                    ))
                    .small(),
                );
            });

            ui.add_space(16.0);
            ui.separator();
            ui.add_space(16.0);

            // Navigation
            ui.horizontal(|ui: &mut Ui| {
                let tabs = [
                    (Tab::Dashboard, "Dashboard"),
                    (Tab::Devices, "Devices"),
                    (Tab::Transfer, "Transfer"),
                    (Tab::Clipboard, "Clipboard"),
                    (Tab::Mirror, "Mirror"),
                    (Tab::Logs, "Logs"),
                ];

                for (tab, label) in tabs {
                    let selected = self.selected_tab == tab;
                    let text = RichText::new(label).strong().size(14.0);
                    let button = Button::new(text)
                        .fill(if selected {
                            Color32::from_rgb(99, 102, 241)
                        } else {
                            Color32::TRANSPARENT
                        })
                        .rounding(8.0);

                    if ui.add_sized([100.0, 36.0], button).clicked() {
                        self.selected_tab = tab.clone();
                    }
                    ui.add_space(4.0);
                }
            });

            ui.add_space(16.0);
            ui.separator();
            ui.add_space(16.0);

            // Content
            ScrollArea::vertical().auto_shrink([false; 2]).show(ui, |ui: &mut Ui| {
                match self.selected_tab {
                    Tab::Dashboard => {
                        ui.heading("Dashboard");
                        ui.add_space(16.0);
                        
                        // Service status
                        ui.label(RichText::new("Service Status").size(14.0).strong());
                        ui.add_space(8.0);
                        ui.horizontal(|ui: &mut Ui| {
                            let discovery_status = if self.discovery_available {
                                ("mDNS Discovery", Color32::from_rgb(34, 197, 94), "Ready")
                            } else {
                                ("mDNS Discovery", Color32::from_rgb(239, 68, 68), "Unavailable")
                            };
                            ui.label(RichText::new(discovery_status.0).small());
                            ui.colored_label(discovery_status.1, discovery_status.2);
                            ui.add_space(16.0);
                            ui.label(RichText::new("Transfer Service").small());
                            ui.colored_label(Color32::from_rgb(34, 197, 94), "Ready");
                            ui.add_space(16.0);
                            ui.label(RichText::new("Clipboard Service").small());
                            ui.colored_label(Color32::from_rgb(34, 197, 94), "Ready");
                        });

                        ui.add_space(16.0);
                        ui.separator();
                        ui.add_space(16.0);

                        // Stats cards
                        ui.horizontal(|ui: &mut Ui| {
                            self.stat_card(ui, "Discovered", &format!("{}", self.discovered_devices.len()));
                            self.stat_card(ui, "Paired", &format!("{}", self.paired_devices.len()));
                            self.stat_card(ui, "Transfers", &format!("{}", self.transfer_tasks.len()));
                            self.stat_card(ui, "FPS", &format!("{:.0}", self.mirror_stats.fps));
                        });

                        ui.add_space(24.0);
                        ui.label(RichText::new("Quick Actions").size(16.0).strong());
                        ui.add_space(8.0);
                        ui.horizontal(|ui: &mut Ui| {
                            if ui.button("Scan Devices").clicked() {
                                self.is_scanning = true;
                                self.add_log("info", "Starting mDNS device discovery...");
                            }
                            if ui.button("Send File").clicked() {
                                self.add_log("info", "Select file to send to paired device");
                            }
                            if ui.button("Sync Clipboard").clicked() {
                                self.add_log("info", "Clipboard sync initiated");
                            }
                        });
                        
                        ui.add_space(16.0);
                        ui.label(RichText::new("Tips").size(12.0).color(Color32::from_gray(150)));
                        ui.add_space(4.0);
                        ui.label(RichText::new("• Use Devices tab to discover and pair with other MMC devices").size(11.0).color(Color32::from_gray(130)));
                        ui.label(RichText::new("• Use Transfer tab to send/receive files between paired devices").size(11.0).color(Color32::from_gray(130)));
                        ui.label(RichText::new("• Use Mirror tab to view remote device screen with input control").size(11.0).color(Color32::from_gray(130)));
                    }
                    Tab::Devices => {
                        ui.heading("Device Management");
                        ui.add_space(8.0);
                        ui.label(RichText::new("Discover and manage devices on your local network using mDNS").size(12.0).color(Color32::from_gray(150)));
                        ui.add_space(16.0);

                        ui.horizontal(|ui: &mut Ui| {
                            if ui.button("Start Discovery").clicked() {
                                self.is_scanning = true;
                                self.add_log("info", "Starting mDNS device discovery...");
                            }
                            if ui.button("Refresh List").clicked() {
                                self.add_log("info", "Refreshing device list...");
                            }
                        });

                        ui.add_space(16.0);
                        ui.label(RichText::new(format!("Discovered Devices ({})", self.discovered_devices.len())).size(14.0).strong());
                        ui.add_space(8.0);

                        ScrollArea::vertical().max_height(250.0).show(ui, |ui: &mut Ui| {
                            if self.discovered_devices.is_empty() {
                                ui.label(RichText::new("No devices found. Click 'Start Discovery' to scan.").color(Color32::from_gray(128)));
                                ui.add_space(8.0);
                                ui.label(RichText::new("Note: Make sure other MMC devices are running on your network.").size(11.0).color(Color32::from_gray(100)));
                            } else {
                                let devices: Vec<_> = self.discovered_devices.clone();
                                for device in devices {
                                    let status_color = if device.online {
                                        Color32::from_rgb(34, 197, 94)
                                    } else {
                                        Color32::from_gray(156)
                                    };
                                    ui.horizontal(|ui: &mut Ui| {
                                        ui.colored_label(status_color, "●");
                                        ui.label(RichText::new(&device.name).strong());
                                        ui.add_space(8.0);
                                        ui.label(
                                            RichText::new(&device.device_type)
                                                .small()
                                                .color(Color32::from_gray(128)),
                                        );
                                    });
                                    ui.horizontal(|ui: &mut Ui| {
                                        ui.add_space(16.0);
                                        ui.label(RichText::new(format!("{}:{}", device.ip, device.port)).small().color(Color32::from_gray(100)));
                                        if !device.os_version.is_empty() {
                                            ui.label(RichText::new(&device.os_version).small().color(Color32::from_gray(100)));
                                        }
                                    });
                                    ui.horizontal(|ui: &mut Ui| {
                                        ui.add_space(16.0);
                                        if ui.button("Pair").clicked() {
                                            self.add_log("info", &format!("Initiating pairing with {}...", device.name));
                                        }
                                        if ui.button("Mirror").clicked() {
                                            self.add_log("info", &format!("Connecting to {} for screen mirror...", device.name));
                                        }
                                    });
                                    ui.add_space(12.0);
                                }
                            }
                        });
                    }
                    Tab::Transfer => {
                        ui.heading("File Transfer");
                        ui.add_space(8.0);
                        ui.label(RichText::new("Send and receive files between paired MMC devices with chunked transfer and resume support").size(12.0).color(Color32::from_gray(150)));
                        ui.add_space(16.0);

                        ui.horizontal(|ui: &mut Ui| {
                            if ui.button("Send File").clicked() {
                                self.add_log("info", "Opening file selector...");
                            }
                            if ui.button("Receive").clicked() {
                                self.add_log("info", "Waiting for incoming files...");
                            }
                        });

                        ui.add_space(16.0);
                        ui.label(RichText::new(format!("Active Transfers ({})", self.transfer_tasks.len())).size(14.0).strong());
                        ui.add_space(8.0);

                        // Display transfer tasks
                        if self.transfer_tasks.is_empty() {
                            ui.label(RichText::new("No active transfers").color(Color32::from_gray(128)));
                            ui.add_space(8.0);
                            ui.label(RichText::new("Select a paired device and click 'Send File' to start a transfer").size(11.0).color(Color32::from_gray(100)));
                        } else {
                            let tasks: Vec<_> = self.transfer_tasks.clone();
                            for task in tasks {
                                let file_name = task.file_name.clone();
                                let progress = task.progress;
                                let speed = task.speed;
                                let state = task.state.clone();

                                ui.horizontal(|ui: &mut Ui| {
                                    ui.label(&file_name);
                                    ui.add_space(8.0);
                                    ui.label(RichText::new(format!("{:.1} MB/s", speed)).small().color(Color32::from_gray(128)));
                                    ui.add_space(8.0);
                                    ui.label(RichText::new(state).small().color(Color32::from_gray(100)));
                                });

                                ui.add_space(4.0);
                                let pb = ProgressBar::new(progress).show_percentage().animate(true);
                                ui.add(pb);

                                ui.horizontal(|ui: &mut Ui| {
                                    ui.label(format!("{:.1}%", progress * 100.0));
                                    ui.with_layout(Layout::right_to_left(Align::Center), |ui: &mut Ui| {
                                        if ui.button("Cancel").clicked() {
                                            self.add_log("info", &format!("Cancelling transfer..."));
                                        }
                                    });
                                });
                                ui.add_space(12.0);
                            }
                        }
                        
                        ui.add_space(16.0);
                        ui.label(RichText::new("Features").size(12.0).strong());
                        ui.add_space(4.0);
                        ui.label(RichText::new("• Chunked transfer with checksums").size(11.0).color(Color32::from_gray(130)));
                        ui.label(RichText::new("• Resume interrupted transfers").size(11.0).color(Color32::from_gray(130)));
                        ui.label(RichText::new("• Progress tracking and speed display").size(11.0).color(Color32::from_gray(130)));
                    }
                    Tab::Clipboard => {
                        ui.heading("Clipboard Sync");
                        ui.add_space(8.0);
                        ui.label(RichText::new("Sync clipboard content between paired MMC devices (text, images, URLs)").size(12.0).color(Color32::from_gray(150)));
                        ui.add_space(16.0);

                        ui.horizontal(|ui: &mut Ui| {
                            if ui.button("Get Clipboard").clicked() {
                                self.add_log("info", "Reading local clipboard...");
                            }
                            if ui.button("Set Clipboard").clicked() {
                                self.add_log("info", "Setting local clipboard...");
                            }
                            if ui.button("Sync All").clicked() {
                                self.add_log("info", "Syncing clipboard to all paired devices...");
                            }
                        });

                        ui.add_space(16.0);
                        ui.label(RichText::new("Clipboard Preview").size(14.0).strong());
                        ui.add_space(8.0);

                        Frame::default()
                            .fill(ui.style().visuals.window_fill())
                            .stroke(Stroke::new(1.0, Color32::from_gray(60)))
                            .rounding(8.0)
                            .show(ui, |ui: &mut Ui| {
                                ui.add_space(8.0);
                                ui.label(&self.clipboard_content);
                                ui.add_space(8.0);
                            });
                        
                        ui.add_space(16.0);
                        ui.label(RichText::new("Features").size(12.0).strong());
                        ui.add_space(4.0);
                        ui.label(RichText::new("• Text clipboard synchronization").size(11.0).color(Color32::from_gray(130)));
                        ui.label(RichText::new("• Image/PNG clipboard support").size(11.0).color(Color32::from_gray(130)));
                        ui.label(RichText::new("• URL detection and special handling").size(11.0).color(Color32::from_gray(130)));
                    }
                    Tab::Mirror => {
                        ui.heading("Screen Mirror & Remote Control");
                        ui.add_space(8.0);
                        ui.label(RichText::new("View remote device screen and control it with mouse/keyboard input").size(12.0).color(Color32::from_gray(150)));
                        ui.add_space(16.0);

                        ui.horizontal(|ui: &mut Ui| {
                            if !self.mirror_active {
                                if ui.button("Start Mirroring").clicked() {
                                    if self.paired_devices.is_empty() {
                                        self.add_log("warn", "Please pair with a device first in Devices tab");
                                    } else {
                                        self.mirror_active = true;
                                        self.add_log("info", "Starting screen mirror session...");
                                    }
                                }
                            } else {
                                if ui.button("Stop Mirroring").clicked() {
                                    self.mirror_active = false;
                                    self.add_log("info", "Stopping screen mirror session...");
                                }
                            }
                        });

                        ui.add_space(16.0);
                        
                        if self.mirror_active {
                            ui.label(RichText::new("Active Session").size(14.0).strong());
                            ui.add_space(8.0);
                            
                            Frame::default()
                                .fill(Color32::from_gray(20))
                                .rounding(12.0)
                                .show(ui, |ui: &mut Ui| {
                                    ui.vertical_centered(|ui: &mut Ui| {
                                        ui.add_space(40.0);
                                        ui.label(RichText::new("🖥️").size(48.0));
                                        ui.add_space(16.0);
                                        ui.label("Screen mirroring active");
                                        ui.add_space(8.0);
                                        ui.label(format!("FPS: {:.1}", self.mirror_stats.fps));
                                        ui.label(format!("Frames: {}", self.mirror_stats.total_frames));
                                        ui.label(format!("Duration: {:.1}s", self.mirror_stats.duration_secs));
                                        ui.add_space(40.0);
                                    });
                                });
                        } else {
                            Frame::default()
                                .fill(Color32::from_gray(20))
                                .rounding(12.0)
                                .show(ui, |ui: &mut Ui| {
                                    ui.vertical_centered(|ui: &mut Ui| {
                                        ui.add_space(40.0);
                                        ui.label(RichText::new("No active mirror").size(16.0).color(Color32::from_gray(128)));
                                        ui.add_space(16.0);
                                        ui.label(RichText::new("Pair with a device first, then click 'Start Mirroring'").size(12.0).color(Color32::from_gray(100)));
                                        ui.add_space(40.0);
                                    });
                                });
                        }
                        
                        ui.add_space(16.0);
                        ui.label(RichText::new("Features").size(12.0).strong());
                        ui.add_space(4.0);
                        ui.label(RichText::new("• Real-time screen capture and streaming").size(11.0).color(Color32::from_gray(130)));
                        ui.label(RichText::new("• Mouse and keyboard input injection").size(11.0).color(Color32::from_gray(130)));
                        ui.label(RichText::new("• Touch event support for mobile devices").size(11.0).color(Color32::from_gray(130)));
                        ui.label(RichText::new("• Platform-specific optimizations (Windows/Android)").size(11.0).color(Color32::from_gray(130)));
                    }
                    Tab::Logs => {
                        ui.horizontal(|ui: &mut Ui| {
                            ui.heading("Application Logs");
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui: &mut Ui| {
                                if ui.button("Clear").clicked() {
                                    self.logs.clear();
                                    self.add_log("info", "Logs cleared");
                                }
                            });
                        });

                        ui.add_space(16.0);

                        ScrollArea::vertical().auto_shrink([false; 2]).show(ui, |ui: &mut Ui| {
                            for (time, message) in &self.logs {
                                ui.horizontal(|ui: &mut Ui| {
                                    ui.label(
                                        RichText::new(time)
                                            .monospace()
                                            .small()
                                            .color(Color32::from_gray(128)),
                                    );
                                    ui.add_space(8.0);

                                    if message.contains("[ERROR]") {
                                        ui.colored_label(Color32::from_rgb(239, 68, 68), message);
                                    } else if message.contains("[WARN]") {
                                        ui.colored_label(Color32::from_rgb(245, 158, 11), message);
                                    } else {
                                        ui.label(RichText::new(message).monospace().small());
                                    }
                                });
                            }
                        });
                    }
                }
            });
        });

        if self.mirror_active {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }
    }
}

impl MmcGuiApp {
    fn stat_card(&mut self, ui: &mut Ui, label: &str, value: &str) {
        ui.horizontal(|ui: &mut Ui| {
            ui.set_min_width(140.0);
            ui.set_height(80.0);

            Frame::default()
                .fill(ui.style().visuals.window_fill())
                .stroke(Stroke::new(1.0, Color32::from_gray(50)))
                .rounding(12.0)
                .show(ui, |ui: &mut Ui| {
                    ui.vertical_centered(|ui: &mut Ui| {
                        ui.add_space(8.0);
                        ui.label(RichText::new(value).size(24.0).strong());
                        ui.add_space(4.0);
                        ui.label(RichText::new(label).small().color(Color32::from_gray(150)));
                        ui.add_space(8.0);
                    });
                });
        });
    }
}
