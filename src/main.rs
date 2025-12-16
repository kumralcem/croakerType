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

#[derive(Parser)]
#[command(name = "croaker")]
#[command(about = "Speech-to-text daemon for Linux/Wayland")]
struct Cli {
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
    /// Interactive configuration wizard
    Configure,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Serve => {
            serve().await?;
        }
        Commands::Toggle => {
            send_command("toggle").await?;
        }
        Commands::Cancel => {
            send_command("cancel").await?;
        }
        Commands::Status => {
            let status = send_command("status").await?;
            println!("{}", status);
        }
        Commands::Configure => {
            configure().await?;
        }
    }

    Ok(())
}

async fn serve() -> anyhow::Result<()> {
    tracing::info!("Starting croaker daemon");

    // Load config
    let config = Config::load()?;

    // Create state machine
    let mut state_machine = StateMachine::new(config.clone())?;
    let event_tx = state_machine.event_sender();

    // Create socket server and state update channel
    let (mut socket_server, state_tx) = SocketServer::new(event_tx.clone());
    
    // Initialize overlay if enabled
    let overlay_tx = if config.overlay.enabled {
        let backend = config.overlay.backend.clone();
        let (tx, rx) = std::sync::mpsc::channel();
        
        // Spawn overlay handler in a regular thread (GTK needs its own thread)
        std::thread::spawn(move || {
            // Create overlay in this thread
            match create_overlay(&backend) {
                Ok(overlay) => {
                    tracing::info!("Overlay initialized with backend: {}", backend);
                    let overlay = std::sync::Arc::new(std::sync::Mutex::new(overlay));
                    while let Ok(state) = rx.recv() {
                        if let Ok(overlay) = overlay.lock() {
                            overlay.update_state(state);
                            match state {
                                DaemonState::Idle => overlay.hide(),
                                _ => overlay.show(),
                            }
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!("Failed to initialize overlay: {} (overlay disabled)", e);
                }
            }
        });
        
        Some(tx)
    } else {
        None
    };
    
    // Connect state updates
    state_machine.set_state_sender(state_tx);
    if let Some(ref overlay_tx) = overlay_tx {
        state_machine.set_overlay_sender(overlay_tx.clone());
    }

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
    let evdev_task = if config.hotkeys.push_to_talk_enabled {
        let event_tx_evdev = event_tx.clone();
        let config_evdev = config.clone();
        Some(tokio::spawn(async move {
            match EvdevMonitor::new(&config_evdev, event_tx_evdev) {
                Ok(mut monitor) => {
                    tracing::info!("Starting evdev push-to-talk monitor");
                    if let Err(e) = monitor.monitor().await {
                        tracing::error!("evdev monitor error: {}", e);
                    }
                }
                Err(e) => {
                    tracing::warn!("Failed to start evdev monitor: {}", e);
                    tracing::warn!("Make sure you're in the 'input' group and have keyboard access");
                }
            }
        }))
    } else {
        None
    };

    // Spawn portal shortcuts monitor (if enabled)
    let portal_task = if config.hotkeys.toggle_enabled {
        let event_tx_portal = event_tx.clone();
        let config_portal = config.clone();
        Some(tokio::spawn(async move {
            match PortalMonitor::new(&config_portal, event_tx_portal).await {
                Ok(mut monitor) => {
                    tracing::info!("Starting portal shortcuts monitor");
                    if let Err(e) = monitor.register_shortcuts().await {
                        tracing::warn!("Portal monitor error: {} (portal shortcuts disabled, push-to-talk still works)", e);
                        tracing::warn!("Portal shortcuts may not be supported on your compositor or may require additional configuration");
                    }
                }
                Err(e) => {
                    tracing::warn!("Failed to start portal monitor: {}", e);
                    tracing::warn!("Portal shortcuts may not be supported on your compositor");
                }
            }
        }))
    } else {
        None
    };

    tracing::info!("Daemon started. Hotkeys are active.");
    tracing::info!("Push-to-talk: {} (key: {})", 
        if config.hotkeys.push_to_talk_enabled { "enabled" } else { "disabled" },
        config.hotkeys.push_to_talk_key);
    tracing::info!("Toggle shortcut: {} (shortcut: {})",
        if config.hotkeys.toggle_enabled { "enabled" } else { "disabled" },
        config.hotkeys.toggle_shortcut);

    // Spawn evdev and portal tasks independently (don't wait for them in select!)
    // They can fail without stopping the daemon
    if let Some(task) = evdev_task {
        tokio::spawn(async move {
            let _ = task.await;
        });
    }
    if let Some(task) = portal_task {
        tokio::spawn(async move {
            let _ = task.await;
        });
    }

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

