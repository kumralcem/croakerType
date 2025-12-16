use crate::daemon::state::{DaemonState, StateEvent};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};
use thiserror::Error;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{UnixListener, UnixStream};

#[derive(Debug, Error)]
pub enum SocketError {
    #[error("Failed to create socket: {0}")]
    CreateError(#[from] std::io::Error),
    #[error("Failed to parse command: {0}")]
    ParseError(String),
}

#[derive(Debug, Clone)]
pub enum Command {
    Toggle,
    Cancel,
    Status,
}

impl Command {
    fn parse(line: &str) -> Result<Self, SocketError> {
        let line = line.trim();
        match line {
            "toggle" => Ok(Command::Toggle),
            "cancel" => Ok(Command::Cancel),
            "status" => Ok(Command::Status),
            _ => Err(SocketError::ParseError(format!("Unknown command: {}", line))),
        }
    }
}

pub struct SocketServer {
    path: PathBuf,
    event_tx: mpsc::Sender<StateEvent>,
    current_state: Arc<Mutex<DaemonState>>,
}

impl SocketServer {
    pub fn new(event_tx: mpsc::Sender<StateEvent>) -> (Self, mpsc::Sender<DaemonState>) {
        let socket_path = Self::socket_path().expect("Failed to get socket path");
        let (state_tx, mut state_rx) = mpsc::channel(1);
        let current_state = Arc::new(Mutex::new(DaemonState::Idle));

        // Spawn task to update current state
        let state_clone = current_state.clone();
        tokio::spawn(async move {
            while let Some(state) = state_rx.recv().await {
                *state_clone.lock().await = state;
            }
        });

        (
            Self {
                path: socket_path,
                event_tx,
                current_state,
            },
            state_tx,
        )
    }

    pub fn socket_path() -> Result<PathBuf, std::io::Error> {
        let cache_dir = dirs::cache_dir()
            .ok_or_else(|| std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "Could not find cache directory"
            ))?;
        
        let croaker_dir = cache_dir.join("croaker");
        std::fs::create_dir_all(&croaker_dir)?;
        
        Ok(croaker_dir.join("croaker.sock"))
    }

    pub async fn listen(&mut self) -> Result<(), SocketError> {
        // Remove existing socket if present
        if self.path.exists() {
            let _ = std::fs::remove_file(&self.path);
        }

        let listener = UnixListener::bind(&self.path)?;
        tracing::info!("Listening on socket: {:?}", self.path);

        loop {
            match listener.accept().await {
                Ok((stream, _)) => {
                    let event_tx = self.event_tx.clone();
                    let current_state = self.current_state.clone();
                    tokio::spawn(async move {
                        if let Err(e) = Self::handle_client(stream, event_tx, current_state).await {
                            tracing::warn!("Client error: {}", e);
                        }
                    });
                }
                Err(e) => {
                    tracing::error!("Accept error: {}", e);
                }
            }
        }
    }

    async fn handle_client(
        mut stream: UnixStream,
        event_tx: mpsc::Sender<StateEvent>,
        current_state: Arc<Mutex<DaemonState>>,
    ) -> Result<(), SocketError> {
        let (read_half, mut write_half) = stream.split();
        let mut reader = BufReader::new(read_half);
        let mut line = String::new();

        reader.read_line(&mut line).await?;
        let command = Command::parse(&line)?;

        match command {
            Command::Toggle => {
                // Send toggle event
                event_tx.send(StateEvent::StartRecording).await
                    .map_err(|e| SocketError::ParseError(e.to_string()))?;
                
                // Wait for state change to determine if we started or stopped
                // For now, just acknowledge
                write_half.write_all(b"ok\n").await?;
            }
            Command::Cancel => {
                event_tx.send(StateEvent::Cancel).await
                    .map_err(|e| SocketError::ParseError(e.to_string()))?;
                write_half.write_all(b"ok\n").await?;
            }
            Command::Status => {
                // Get current state
                let state = *current_state.lock().await;
                let state_str = format!("{:?}\n", state);
                write_half.write_all(state_str.as_bytes()).await?;
            }
        }

        Ok(())
    }
}

