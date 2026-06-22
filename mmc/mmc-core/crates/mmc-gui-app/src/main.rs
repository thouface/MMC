//! MMC GUI Application
//!
//! A cross-platform desktop GUI application for Multi-Device Communication.

mod app;
mod http_server;
mod platform;

use std::sync::{Arc, atomic::AtomicBool};

use app::MmcGuiApp;
use http_server::start_server;
use platform::get_platform_info;

use eframe::{egui, Renderer};

pub const APP_NAME: &str = "MMC Control Center";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

fn main() -> eframe::Result {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(tracing::Level::INFO.into()),
        )
        .init();

    tracing::info!("Starting {} v{}", APP_NAME, VERSION);

    // Get platform info
    let platform_info = get_platform_info();

    // Start embedded HTTP server for HTML GUI
    let server_running = Arc::new(AtomicBool::new(true));
    let server_port = start_server(server_running.clone());

    tracing::info!("Embedded HTTP server started on port {}", server_port);

    // Setup panic hook for logging
    let default_panic = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        tracing::error!("Application panic: {}", panic_info);
        default_panic(panic_info);
    }));

    // Application options
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1200.0, 800.0])
            .with_min_inner_size([800.0, 600.0])
            .with_title(format!("{} v{}", APP_NAME, VERSION))
            .with_resizable(true)
            .with_fullscreen(false)
            .with_decorations(true)
            .with_transparent(false),
        renderer: Renderer::Glow,
        shader_version: None,
        centered: true,
        ..Default::default()
    };

    // Create and run the application
    let app = MmcGuiApp::new(platform_info, server_port);
    eframe::run_native(
        APP_NAME,
        options,
        Box::new(|_cc| Ok(Box::new(app))),
    )
}
