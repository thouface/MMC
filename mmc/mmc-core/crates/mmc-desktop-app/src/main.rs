//! MMC Desktop Application Entry Point
//!
//! A CLI application for testing MMC functionality on desktop platforms.

use clap::{Parser, Subcommand};
use mmc_desktop_app::{AppState, get_platform_type};
use mmc_desktop_app::commands::{DeviceCommand, TransferCommand, ClipboardCommand, MirrorCommand};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, Layer};

#[derive(Parser)]
#[command(name = "mmc-desktop")]
#[command(about = "MMC Desktop Application - Multi-Device Communication", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Device discovery and pairing commands
    Device {
        #[command(subcommand)]
        command: DeviceCommand,
    },
    /// File transfer commands
    Transfer {
        #[command(subcommand)]
        command: TransferCommand,
    },
    /// Clipboard synchronization commands
    Clipboard {
        #[command(subcommand)]
        command: ClipboardCommand,
    },
    /// Screen mirroring commands
    Mirror {
        #[command(subcommand)]
        command: MirrorCommand,
    },
    /// Start interactive mode (TUI-like REPL)
    Interactive,
    /// Show current platform info
    Info,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer().with_filter(
            tracing_subscriber::filter::LevelFilter::INFO,
        ))
        .init();

    let cli = Cli::parse();
    let mut state = AppState::new();
    state.init()?;

    match cli.command {
        Commands::Device { command } => {
            commands::handle_device_command(&mut state, command).await?;
        }
        Commands::Transfer { command } => {
            commands::handle_transfer_command(&mut state, command).await?;
        }
        Commands::Clipboard { command } => {
            commands::handle_clipboard_command(&state, command).await?;
        }
        Commands::Mirror { command } => {
            commands::handle_mirror_command(&mut state, command).await?;
        }
        Commands::Interactive => {
            commands::run_interactive_mode(&mut state).await?;
        }
        Commands::Info => {
            println!("MMC Desktop Application");
            println!("Platform: {}", get_platform_type());
            println!("Device ID: {}", state.device_id());
            println!("Device Name: {}", state.device_name());
            println!("Version: {}", env!("CARGO_PKG_VERSION"));
        }
    }

    Ok(())
}

mod commands {
    use mmc_desktop_app::{AppState, Result};
    use mmc_desktop_app::commands::{DeviceCommand, TransferCommand, ClipboardCommand, MirrorCommand};
    use std::io::{self, BufRead, Write};

    pub async fn handle_device_command(state: &mut AppState, command: DeviceCommand) -> Result<()> {
        match command {
            DeviceCommand::Discover => {
                println!("Starting device discovery...");
                let devices = state.discover_devices().await?;
                if devices.is_empty() {
                    println!("No devices found.");
                } else {
                    println!("Found {} devices:", devices.len());
                    for (i, device) in devices.iter().enumerate() {
                        println!("  {}. {} ({}) @ {}:{}", i + 1, device.name, device.id, device.ip, device.port);
                    }
                }
            }
            DeviceCommand::Pair { device_id } => {
                println!("Pairing with device: {}", device_id);
                let success = state.pair_device(&device_id).await?;
                if success {
                    println!("Pairing successful!");
                } else {
                    println!("Pairing failed.");
                }
            }
            DeviceCommand::List => {
                let paired = state.get_paired_devices();
                if paired.is_empty() {
                    println!("No paired devices.");
                } else {
                    println!("Paired devices:");
                    for device in paired {
                        println!("  - {} ({}) @ {}:{}", device.name, device.id, device.ip, device.port);
                    }
                }
            }
            DeviceCommand::Unpair { device_id } => {
                println!("Unpairing device: {}", device_id);
                state.unpair_device(&device_id)?;
                println!("Device unpaired.");
            }
        }
        Ok(())
    }

    pub async fn handle_transfer_command(state: &mut AppState, command: TransferCommand) -> Result<()> {
        match command {
            TransferCommand::Send { device_id, file } => {
                println!("Sending file '{}' to device {}...", file, device_id);
                let task_id = state.send_file(&device_id, &file).await?;
                println!("Transfer started. Task ID: {}", task_id);
            }
            TransferCommand::List => {
                let tasks = state.get_transfer_tasks();
                if tasks.is_empty() {
                    println!("No active transfers.");
                } else {
                    println!("Active transfers:");
                    for task in tasks {
                        println!("  - {} ({}) [{}]", task.file_name, task.task_id, task.state);
                    }
                }
            }
            TransferCommand::Cancel { task_id } => {
                println!("Canceling transfer: {}", task_id);
                state.cancel_transfer(&task_id)?;
                println!("Transfer canceled.");
            }
        }
        Ok(())
    }

    pub async fn handle_clipboard_command(state: &AppState, command: ClipboardCommand) -> Result<()> {
        match command {
            ClipboardCommand::Get => {
                let content = state.get_clipboard_content().await?;
                match content {
                    Some(text) => println!("Clipboard content: {}", text),
                    None => println!("Clipboard is empty or contains non-text data."),
                }
            }
            ClipboardCommand::Set { text } => {
                println!("Setting clipboard content: {}", text);
                state.set_clipboard_content(&text).await?;
                println!("Clipboard updated.");
            }
            ClipboardCommand::Sync { device_id } => {
                println!("Syncing clipboard with device: {}", device_id);
                state.sync_clipboard(&device_id).await?;
                println!("Clipboard synced.");
            }
            ClipboardCommand::Monitor { duration_secs } => {
                println!("Monitoring clipboard for {} seconds...", duration_secs);
                state.monitor_clipboard(duration_secs).await?;
            }
        }
        Ok(())
    }

    pub async fn handle_mirror_command(state: &mut AppState, command: MirrorCommand) -> Result<()> {
        match command {
            MirrorCommand::Start { device_id } => {
                println!("Starting screen mirroring with device: {}", device_id);
                state.start_mirror(&device_id).await?;
                println!("Screen mirroring started.");
            }
            MirrorCommand::Stop => {
                println!("Stopping screen mirroring...");
                state.stop_mirror()?;
                println!("Screen mirroring stopped.");
            }
            MirrorCommand::Status => {
                let stats = state.get_mirror_stats();
                match stats {
                    Some(stats) => {
                        println!("Mirror session status:");
                        println!("  State: {}", stats.state);
                        println!("  Video frames: {}", stats.video_frames);
                        println!("  Audio frames: {}", stats.audio_frames);
                        println!("  Input events: {}", stats.input_events);
                        if let Some(duration) = stats.duration {
                            println!("  Duration: {:.1}s", duration);
                        }
                    }
                    None => println!("No active mirror session."),
                }
            }
        }
        Ok(())
    }

    pub async fn run_interactive_mode(state: &mut AppState) -> Result<()> {
        println!("MMC Interactive Mode");
        println!("Type 'help' for available commands, 'quit' to exit.");
        println!("Platform: {}", mmc_desktop_app::get_platform_type());
        println!();

        let stdin = io::stdin();
        let mut stdout = io::stdout();

        loop {
            print!("mmc> ");
            stdout.flush().unwrap();

            let mut line = String::new();
            stdin.lock().read_line(&mut line).unwrap();
            let line = line.trim();

            if line.is_empty() {
                continue;
            }

            match line {
                "quit" | "exit" => {
                    println!("Goodbye!");
                    break;
                }
                "help" => {
                    println!("Available commands:");
                    println!("  discover          - Discover nearby devices");
                    println!("  pair <device_id>  - Pair with a device");
                    println!("  devices           - List paired devices");
                    println!("  unpair <device_id> - Unpair a device");
                    println!("  send <device_id> <file> - Send a file");
                    println!("  transfers         - List active transfers");
                    println!("  cancel <task_id>  - Cancel a transfer");
                    println!("  clipboard get     - Get clipboard content");
                    println!("  clipboard set <text> - Set clipboard content");
                    println!("  mirror <device_id> - Start screen mirroring");
                    println!("  mirror stop       - Stop screen mirroring");
                    println!("  info              - Show device info");
                    println!("  quit              - Exit interactive mode");
                }
                "discover" => {
                    let devices = state.discover_devices().await?;
                    if devices.is_empty() {
                        println!("No devices found.");
                    } else {
                        for (i, device) in devices.iter().enumerate() {
                            println!("{}. {} ({})", i + 1, device.name, device.id);
                        }
                    }
                }
                "devices" => {
                    let paired = state.get_paired_devices();
                    if paired.is_empty() {
                        println!("No paired devices.");
                    } else {
                        for device in paired {
                            println!("{} ({})", device.name, device.id);
                        }
                    }
                }
                "info" => {
                    println!("Device ID: {}", state.device_id());
                    println!("Device Name: {}", state.device_name());
                    println!("Platform: {}", mmc_desktop_app::get_platform_type());
                }
                cmd if cmd.starts_with("pair ") => {
                    let device_id = cmd.strip_prefix("pair ").unwrap();
                    let success = state.pair_device(device_id).await?;
                    println!("Pairing: {}", if success { "success" } else { "failed" });
                }
                cmd if cmd.starts_with("unpair ") => {
                    let device_id = cmd.strip_prefix("unpair ").unwrap();
                    state.unpair_device(device_id)?;
                    println!("Device unpaired.");
                }
                cmd if cmd.starts_with("send ") => {
                    let args: Vec<&str> = cmd.strip_prefix("send ").unwrap().split_whitespace().collect();
                    if args.len() < 2 {
                        println!("Usage: send <device_id> <file_path>");
                    } else {
                        let task_id = state.send_file(args[0], args[1]).await?;
                        println!("Transfer started: {}", task_id);
                    }
                }
                cmd if cmd.starts_with("clipboard set ") => {
                    let text = cmd.strip_prefix("clipboard set ").unwrap();
                    state.set_clipboard_content(text).await?;
                    println!("Clipboard set.");
                }
                "clipboard get" => {
                    let content = state.get_clipboard_content().await?;
                    println!("Clipboard: {}", content.unwrap_or_default());
                }
                cmd if cmd.starts_with("mirror ") && cmd != "mirror stop" => {
                    let device_id = cmd.strip_prefix("mirror ").unwrap();
                    state.start_mirror(device_id).await?;
                    println!("Mirroring started.");
                }
                "mirror stop" => {
                    state.stop_mirror()?;
                    println!("Mirroring stopped.");
                }
                "transfers" => {
                    let tasks = state.get_transfer_tasks();
                    for task in tasks {
                        println!("{}: {} [{}]", task.task_id, task.file_name, task.state);
                    }
                }
                _ => {
                    println!("Unknown command: {}. Type 'help' for available commands.", line);
                }
            }
        }

        Ok(())
    }
}