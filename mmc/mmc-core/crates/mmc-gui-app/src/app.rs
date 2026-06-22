//! MMC GUI Application Module

use std::collections::VecDeque;

use eframe::egui::{Align, Button, CentralPanel, Color32, Context, Frame, Layout, ProgressBar, RichText, ScrollArea, Stroke, Ui, Visuals};
use serde::{Deserialize, Serialize};

use crate::platform::PlatformInfo;
use crate::{APP_NAME, VERSION};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Device {
    pub id: String,
    pub name: String,
    pub device_type: String,
    pub ip: String,
    pub port: u16,
    pub online: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransferTask {
    pub task_id: String,
    pub file_name: String,
    pub progress: f32,
    pub speed: f32,
    pub state: String,
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
    discovered_devices: Vec<Device>,
    paired_devices: Vec<Device>,
    selected_device: Option<usize>,
    transfer_tasks: Vec<TransferTask>,
    simulated_progress: f32,
    simulated_speed: f32,
    clipboard_content: String,
    mirror_active: bool,
    mirror_stats: MirrorStats,
    logs: VecDeque<(String, String)>,
    max_logs: usize,
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

        Self {
            platform_info,
            server_port,
            selected_tab: Tab::Dashboard,
            show_settings: false,
            discovered_devices: vec![
                Device {
                    id: "device-001".to_string(),
                    name: "Xiaomi 14 Pro".to_string(),
                    device_type: "Android".to_string(),
                    ip: "192.168.1.101".to_string(),
                    port: 8765,
                    online: true,
                },
                Device {
                    id: "device-002".to_string(),
                    name: "ThinkPad X1 Carbon".to_string(),
                    device_type: "Windows".to_string(),
                    ip: "192.168.1.102".to_string(),
                    port: 8765,
                    online: true,
                },
                Device {
                    id: "device-003".to_string(),
                    name: "Apple TV 4K".to_string(),
                    device_type: "tvOS".to_string(),
                    ip: "192.168.1.103".to_string(),
                    port: 8765,
                    online: false,
                },
            ],
            paired_devices: vec![],
            selected_device: None,
            transfer_tasks: vec![TransferTask {
                task_id: "task-001".to_string(),
                file_name: "document.pdf".to_string(),
                progress: 0.67,
                speed: 12.4,
                state: "transferring".to_string(),
            }],
            simulated_progress: 0.67,
            simulated_speed: 12.4,
            clipboard_content: String::from("\"Hello World from MMC!\""),
            mirror_active: false,
            mirror_stats: MirrorStats {
                fps: 30.0,
                total_frames: 1250,
                duration_secs: 41.7,
            },
            logs,
            max_logs: 100,
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
        if self.simulated_progress < 1.0 {
            self.simulated_progress += 0.001;
            self.simulated_speed = 10.0 + (rand::random::<f32>() * 5.0);
            if self.simulated_progress > 1.0 {
                self.simulated_progress = 1.0;
                self.add_log("info", "Transfer completed: document.pdf");
            }
        }

        if self.mirror_active {
            self.mirror_stats.total_frames += 1;
            self.mirror_stats.duration_secs += 1.0 / 60.0;
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
                    let mut button = Button::new(text)
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
                            if ui.button("🔍 Scan").clicked() {
                                self.add_log("info", "Scanning for devices...");
                            }
                            if ui.button("📤 Send").clicked() {
                                self.add_log("info", "Opening file dialog...");
                            }
                            if ui.button("📋 Sync").clicked() {
                                self.add_log("info", "Clipboard sync initiated");
                            }
                        });
                    }
                    Tab::Devices => {
                        ui.heading("Device Management");
                        ui.add_space(16.0);

                        ui.horizontal(|ui: &mut Ui| {
                            if ui.button("🔍 Scan").clicked() {
                                self.add_log("info", "Scanning for devices...");
                            }
                            if ui.button("🔄 Refresh").clicked() {
                                self.add_log("info", "Device list refreshed");
                            }
                        });

                        ui.add_space(16.0);
                        ui.label(RichText::new("Discovered Devices").size(14.0).strong());
                        ui.add_space(8.0);

                        ScrollArea::vertical().max_height(250.0).show(ui, |ui: &mut Ui| {
                            // Clone devices to avoid borrow issues
                            let devices: Vec<_> = self.discovered_devices.clone();
                            let selected = self.selected_device.unwrap_or(999);

                            for (i, device) in devices.iter().enumerate() {
                                let device_name = device.name.clone();
                                let device_type = device.device_type.clone();
                                let device_ip = device.ip.clone();
                                let device_port = device.port;
                                let status_color = if device.online {
                                    Color32::from_rgb(34, 197, 94)
                                } else {
                                    Color32::from_gray(156)
                                };
                                ui.horizontal(|ui: &mut Ui| {
                                    ui.colored_label(status_color, "●");
                                    ui.label(&device_name);
                                    ui.label(
                                        RichText::new(&device_type)
                                            .small()
                                            .color(Color32::from_gray(128)),
                                    );
                                    ui.label(
                                        RichText::new(format!("{}:{}", device_ip, device_port))
                                            .small()
                                            .color(Color32::from_gray(128)),
                                    );
                                });

                                if i == selected {
                                    let name_for_log = device_name.clone();
                                    ui.horizontal(|ui: &mut Ui| {
                                        ui.add_space(24.0);
                                        if ui.button("🔗 Pair").clicked() {
                                            self.add_log("info", &format!("Pairing with {}...", name_for_log));
                                        }
                                    });
                                }
                                ui.add_space(4.0);
                            }
                        });
                    }
                    Tab::Transfer => {
                        ui.heading("File Transfer");
                        ui.add_space(16.0);

                        ui.horizontal(|ui: &mut Ui| {
                            if ui.button("📤 Send File").clicked() {
                                self.add_log("info", "Opening file selector...");
                            }
                            if ui.button("📥 Receive").clicked() {
                                self.add_log("info", "Waiting for incoming files...");
                            }
                        });

                        ui.add_space(24.0);
                        ui.label(RichText::new("Active Transfers").size(14.0).strong());
                        ui.add_space(8.0);

                        // Display transfer tasks
                        let tasks: Vec<_> = self.transfer_tasks.clone();
                        for task in tasks {
                            let task_id = task.task_id.clone();
                            let file_name = task.file_name.clone();
                            let progress = task.progress;
                            let speed = task.speed;

                            ui.horizontal(|ui: &mut Ui| {
                                ui.label("📄");
                                ui.label(&file_name);
                                ui.label(format!("{:.1} MB/s", speed));
                            });

                            ui.add_space(4.0);
                            let pb = ProgressBar::new(progress).show_percentage().animate(true);
                            ui.add(pb);

                            let task_id_for_log = task_id.clone();
                            ui.horizontal(|ui: &mut Ui| {
                                ui.label(format!("{:.1}%", progress * 100.0));
                                ui.with_layout(Layout::right_to_left(Align::Center), |ui: &mut Ui| {
                                    if ui.button("❌ Cancel").clicked() {
                                        self.add_log("info", &format!("Cancelling {}...", task_id_for_log));
                                    }
                                });
                            });
                            ui.add_space(12.0);
                        }
                    }
                    Tab::Clipboard => {
                        ui.heading("Clipboard Sync");
                        ui.add_space(16.0);

                        ui.horizontal(|ui: &mut Ui| {
                            if ui.button("📋 Get").clicked() {
                                self.add_log("info", "Getting clipboard content...");
                            }
                            if ui.button("📝 Set").clicked() {
                                self.add_log("info", "Setting clipboard content...");
                            }
                            if ui.button("🔄 Sync All").clicked() {
                                self.add_log("info", "Syncing clipboard...");
                            }
                        });

                        ui.add_space(24.0);
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
                    }
                    Tab::Mirror => {
                        ui.heading("Screen Mirror");
                        ui.add_space(16.0);

                        ui.horizontal(|ui: &mut Ui| {
                            if !self.mirror_active {
                                if ui.button("▶️ Start Mirroring").clicked() {
                                    self.mirror_active = true;
                                    self.add_log("info", "Starting screen mirror...");
                                }
                            } else {
                                if ui.button("⏹️ Stop Mirroring").clicked() {
                                    self.mirror_active = false;
                                    self.add_log("info", "Stopping screen mirror...");
                                }
                            }
                        });

                        ui.add_space(24.0);

                        Frame::default()
                            .fill(Color32::from_gray(20))
                            .rounding(12.0)
                            .show(ui, |ui: &mut Ui| {
                                ui.vertical_centered(|ui: &mut Ui| {
                                    ui.add_space(40.0);
                                    ui.label(RichText::new("🖥️").size(48.0));
                                    ui.add_space(16.0);
                                    if self.mirror_active {
                                        ui.label("Screen mirroring active");
                                        ui.add_space(8.0);
                                        ui.label(format!("FPS: {:.1}", self.mirror_stats.fps));
                                        ui.label(format!("Frames: {}", self.mirror_stats.total_frames));
                                        ui.label(format!("Duration: {:.1}s", self.mirror_stats.duration_secs));
                                    } else {
                                        ui.label(RichText::new("No active mirror").color(Color32::from_gray(128)));
                                    }
                                    ui.add_space(40.0);
                                });
                            });
                    }
                    Tab::Logs => {
                        ui.horizontal(|ui: &mut Ui| {
                            ui.heading("Application Logs");
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui: &mut Ui| {
                                if ui.button("🗑️ Clear").clicked() {
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

        if self.mirror_active || self.simulated_progress < 1.0 {
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
