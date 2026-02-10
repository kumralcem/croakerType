mod audio;
mod config;
mod daemon;
mod input;
mod output;
mod overlay;
mod transcribe;

use clap::{Parser, Subcommand};
use config::Config;
use daemon::state::{DaemonState, StateEvent, StateMachine};
use input::{evdev::EvdevMonitor, portal::PortalMonitor, socket::SocketServer};
use overlay::create_overlay;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt};
use tokio::net::UnixStream;
use tokio::time::{sleep, Duration};

#[derive(Parser)]
#[command(name = "croaker")]
#[command(about = "Speech-to-text daemon for Linux/Wayland")]
struct Cli {
    /// Enable debug logging
    #[arg(long, global = true)]
    debug: bool,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Start the daemon
    Serve,
    /// Toggle recording on/off
    Toggle,
    /// Cancel current operation
    Cancel,
    /// Get current status
    Status,
    /// Toggle output mode (direct/clipboard/both)
    ToggleOutputMode,
    /// Toggle language (cycles through configured languages)
    ToggleLanguage,
    /// Interactive configuration wizard
    Configure,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    let env_filter = if cli.debug {
        tracing_subscriber::EnvFilter::new("debug")
    } else {
        tracing_subscriber::EnvFilter::from_default_env()
    };

    tracing_subscriber::fmt()
        .with_env_filter(env_filter)
        .init();

    match cli.command {
        Commands::Serve => {
            serve()?;
        }
        Commands::Toggle => {
            tokio::runtime::Runtime::new()?.block_on(send_command("toggle"))?;
        }
        Commands::Cancel => {
            tokio::runtime::Runtime::new()?.block_on(send_command("cancel"))?;
        }
        Commands::Status => {
            let status = tokio::runtime::Runtime::new()?.block_on(send_command("status"))?;
            println!("{}", status);
        }
        Commands::ToggleOutputMode => {
            tokio::runtime::Runtime::new()?.block_on(send_command("toggle-output-mode"))?;
        }
        Commands::ToggleLanguage => {
            tokio::runtime::Runtime::new()?.block_on(send_command("toggle-language"))?;
        }
        Commands::Configure => {
            tokio::runtime::Runtime::new()?.block_on(configure())?;
        }
    }

    Ok(())
}

fn serve() -> anyhow::Result<()> {
    tracing::info!("Starting croaker daemon");

    // Load config
    let config = Config::load()?;
    tracing::info!("Config loaded, push_to_talk_enabled: {}", config.hotkeys.push_to_talk_enabled);

    let backend = config.overlay.backend.clone();
    let overlay_enabled = config.overlay.enabled;
    
    // Create message channel for overlay/tray
    let (overlay_tx, overlay_rx) = std::sync::mpsc::channel::<crate::overlay::OverlayMessage>();
    
    // Spawn the daemon logic in a background thread with its own tokio runtime
    let config_clone = config.clone();
    let overlay_tx_clone = overlay_tx.clone();
    std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().expect("Failed to create tokio runtime");
        rt.block_on(async move {
            if let Err(e) = run_daemon(config_clone, overlay_tx_clone).await {
                tracing::error!("Daemon error: {}", e);
            }
        });
    });
    
    // Run tray/overlay on main thread
    if overlay_enabled && (backend == "tray" || backend == "auto") {
        tracing::info!("Starting system tray");
        match overlay::run_tray(overlay_rx) {
            Ok(_) => {
                tracing::info!("Tray exited normally");
            }
            Err(e) => {
                tracing::error!("Tray error: {}", e);
                tracing::warn!("Continuing without tray - daemon will still work, just without visual feedback");
                // Continue running without tray - daemon functionality still works
                // Block main thread forever to keep process alive
                loop {
                    std::thread::sleep(std::time::Duration::from_secs(3600));
                }
            }
        }
    } else if overlay_enabled && backend == "notification" {
        // For notification backend, process messages in a loop
        match create_overlay(&backend) {
            Ok(overlay) => {
                tracing::info!("Overlay initialized with backend: {}", backend);
                while let Ok(msg) = overlay_rx.recv() {
                    match msg {
                        crate::overlay::OverlayMessage::State(state) => {
                            overlay.update_state(state);
                            match state {
                                DaemonState::Idle => overlay.hide(),
                                _ => overlay.show(),
                            }
                        }
                        crate::overlay::OverlayMessage::OutputMode(mode) => {
                            overlay.update_output_mode(&mode);
                        }
                        crate::overlay::OverlayMessage::Language(lang) => {
                            overlay.update_language(&lang);
                        }
                        crate::overlay::OverlayMessage::AudioLevel(level) => {
                            overlay.update_audio_level(level);
                        }
                        crate::overlay::OverlayMessage::Show => {
                            overlay.show();
                        }
                        crate::overlay::OverlayMessage::Hide => {
                            overlay.hide();
                        }
                    }
                }
            }
            Err(e) => {
                tracing::warn!("Failed to initialize overlay: {} (overlay disabled)", e);
                // Block main thread forever
                loop {
                    std::thread::sleep(std::time::Duration::from_secs(3600));
                }
            }
        }
    } else {
        // No overlay/tray, just block main thread
        loop {
            std::thread::sleep(std::time::Duration::from_secs(3600));
        }
    }

    Ok(())
}

async fn run_daemon(config: Config, overlay_tx: std::sync::mpsc::Sender<crate::overlay::OverlayMessage>) -> anyhow::Result<()> {
    // Create state machine
    let mut state_machine = StateMachine::new(config.clone())?;
    let event_tx = state_machine.event_sender();

    // Create socket server and state update channel
    let (mut socket_server, state_tx) = SocketServer::new(event_tx.clone());
    
    // Connect state updates
    state_machine.set_state_sender(state_tx);
    state_machine.set_overlay_sender(overlay_tx.clone());
    
    // Send initial mode and language to overlay
    let initial_mode = match config.output.output_mode {
        crate::config::OutputMode::Direct => "Direct",
        crate::config::OutputMode::Clipboard => "Clipboard",
        crate::config::OutputMode::Both => "Both",
    };
    let _ = overlay_tx.send(crate::overlay::OverlayMessage::OutputMode(initial_mode.to_string()));
    let _ = overlay_tx.send(crate::overlay::OverlayMessage::Language(config.general.language.clone()));

    // Spawn state machine task
    let state_machine_task = tokio::spawn(async move {
        if let Err(e) = state_machine.run().await {
            tracing::error!("State machine error: {}", e);
        }
    });

    // Spawn socket server task
    let socket_task = tokio::spawn(async move {
        if let Err(e) = socket_server.listen().await {
            tracing::error!("Socket server error: {}", e);
        }
    });

    // Spawn evdev push-to-talk monitor (if enabled)
    if config.hotkeys.push_to_talk_enabled {
        let event_tx_evdev = event_tx.clone();
        let config_evdev = config.clone();
        tokio::spawn(async move {
            loop {
                match EvdevMonitor::new(&config_evdev, event_tx_evdev.clone()) {
                    Ok(mut monitor) => {
                        tracing::info!("Starting evdev push-to-talk monitor");
                        if let Err(e) = monitor.monitor().await {
                            tracing::warn!(
                                "evdev monitor stopped: {}. Retrying in 5 seconds...",
                                e
                            );
                        } else {
                            tracing::warn!("evdev monitor ended unexpectedly. Retrying in 5 seconds...");
                        }
                    }
                    Err(e) => {
                        tracing::warn!(
                            "Failed to start evdev monitor: {}. Retrying in 5 seconds...",
                            e
                        );
                    }
                }
                sleep(Duration::from_secs(5)).await;
            }
        });
    }

    // Spawn portal shortcuts monitor (if enabled)
    if config.hotkeys.toggle_enabled {
        let event_tx_portal = event_tx.clone();
        let config_portal = config.clone();
        tokio::spawn(async move {
            loop {
                match PortalMonitor::new(&config_portal, event_tx_portal.clone()).await {
                    Ok(mut monitor) => {
                        tracing::info!("Starting portal shortcuts monitor");
                        if let Err(e) = monitor.register_shortcuts().await {
                            tracing::warn!(
                                "Portal monitor error: {}. Retrying in 5 seconds...",
                                e
                            );
                        } else {
                            tracing::warn!("Portal monitor ended unexpectedly. Retrying in 5 seconds...");
                        }
                    }
                    Err(e) => {
                        tracing::warn!(
                            "Failed to start portal monitor: {}. Retrying in 5 seconds...",
                            e
                        );
                    }
                }
                sleep(Duration::from_secs(5)).await;
            }
        });
    }

    tracing::info!("Daemon started. Hotkeys are active.");
    tracing::info!("Push-to-talk: {} (key: {})", 
        if config.hotkeys.push_to_talk_enabled { "enabled" } else { "disabled" },
        config.hotkeys.push_to_talk_key);
    tracing::info!("Toggle shortcut: {} (shortcut: {})",
        if config.hotkeys.toggle_enabled { "enabled" } else { "disabled" },
        config.hotkeys.toggle_shortcut);

    // Wait for core tasks (state machine and socket server)
    tokio::select! {
        _ = state_machine_task => {
            tracing::error!("State machine task ended");
        }
        _ = socket_task => {
            tracing::error!("Socket server task ended");
        }
    }

    Ok(())
}

async fn send_command(cmd: &str) -> anyhow::Result<String> {
    let socket_path = SocketServer::socket_path()?;

    if !socket_path.exists() {
        anyhow::bail!("Daemon is not running (socket not found)");
    }

    let mut stream = UnixStream::connect(&socket_path).await?;
    stream.write_all(cmd.as_bytes()).await?;
    stream.write_all(b"\n").await?;

    let mut response = String::new();
    let mut reader = tokio::io::BufReader::new(stream);
    reader.read_line(&mut response).await?;

    Ok(response.trim().to_string())
}

async fn configure() -> anyhow::Result<()> {
    println!("croaker Configuration Wizard");
    println!("============================");
    println!();

    // Check API key
    let config = Config::load()?;
    match config.load_api_key() {
        Ok(_) => println!("✓ API key found"),
        Err(_) => {
            println!("✗ API key not found");
            println!("Please create ~/.config/croaker/groq.key and add your Groq API key");
            println!("Make sure to set permissions: chmod 600 ~/.config/croaker/groq.key");
        }
    }

    // Check group membership
    println!();
    println!("Checking permissions...");
    // Check if user can access /dev/uinput (indirect check for input group)
    let can_access_uinput = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open("/dev/uinput")
        .is_ok();

    if can_access_uinput {
        println!("✓ User has access to /dev/uinput (likely in 'input' group)");
    } else {
        println!("✗ User does NOT have access to /dev/uinput");
        println!("Run: sudo usermod -aG input $USER");
        println!("Then log out and back in");
    }

    // Check dependencies
    println!();
    println!("Checking dependencies...");
    
    for (cmd, name) in [("pw-record", "PipeWire"), ("wl-copy", "wl-clipboard")] {
        if which::which(cmd).is_ok() {
            println!("✓ {} found", name);
        } else {
            println!("✗ {} not found", name);
            println!("Install with: sudo dnf install pipewire-utils wl-clipboard");
        }
    }

    Ok(())
}

