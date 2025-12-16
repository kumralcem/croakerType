use crate::config::Config;
use crate::daemon::state::StateEvent;
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::mpsc;
use zbus::dbus_proxy;
use zbus::Connection;

#[derive(Debug, Error)]
pub enum PortalError {
    #[error("D-Bus connection error: {0}")]
    ConnectionError(#[from] zbus::Error),
    #[error("Failed to register shortcut: {0}")]
    RegisterError(String),
}

#[dbus_proxy(
    interface = "org.freedesktop.portal.GlobalShortcuts",
    default_service = "org.freedesktop.portal.Desktop",
    default_path = "/org/freedesktop/portal/desktop"
)]
trait GlobalShortcuts {
    fn create_session(
        &self,
        options: std::collections::HashMap<&str, zbus::zvariant::Value<'_>>,
    ) -> zbus::Result<zbus::zvariant::OwnedObjectPath>;

    fn list_shortcuts(&self, session_handle: &zbus::zvariant::ObjectPath<'_>) -> zbus::Result<Vec<(String, Vec<String>)>>;

    fn bind_shortcuts(
        &self,
        session_handle: &zbus::zvariant::ObjectPath<'_>,
        shortcuts: std::collections::HashMap<&str, std::collections::HashMap<&str, zbus::zvariant::Value<'_>>>,
        parent_window: &str,
    ) -> zbus::Result<zbus::zvariant::OwnedObjectPath>;

    #[dbus_proxy(signal)]
    fn activated(&self, shortcut: &str, timestamp: u64, options: std::collections::HashMap<&str, zbus::zvariant::Value<'_>>) -> zbus::Result<()>;
}

pub struct PortalMonitor {
    connection: Connection,
    event_tx: mpsc::Sender<StateEvent>,
    toggle_shortcut: String,
    cancel_shortcut: String,
}

impl PortalMonitor {
    pub async fn new(config: &Config, event_tx: mpsc::Sender<StateEvent>) -> Result<Self, PortalError> {
        let connection = Connection::session().await?;

        Ok(Self {
            connection,
            event_tx,
            toggle_shortcut: config.hotkeys.toggle_shortcut.clone(),
            cancel_shortcut: config.hotkeys.cancel_shortcut.clone(),
        })
    }

    pub async fn register_shortcuts(&mut self) -> Result<(), PortalError> {
        let proxy = GlobalShortcutsProxy::new(&self.connection).await?;

        // Create session - handle_token is optional per freedesktop portal spec
        // Try without it first, as some implementations (like GNOME) may have issues with it
        let options = std::collections::HashMap::new();
        
        let session_handle = proxy.create_session(options).await?;
        tracing::info!("Created portal session: {:?}", session_handle);

        // Bind shortcuts
        let mut shortcuts = std::collections::HashMap::new();
        
        let mut toggle_binding = std::collections::HashMap::new();
        toggle_binding.insert("shortcut", zbus::zvariant::Value::new(self.toggle_shortcut.clone()));
        toggle_binding.insert("description", zbus::zvariant::Value::new("Toggle recording"));
        shortcuts.insert("toggle", toggle_binding);

        let mut cancel_binding = std::collections::HashMap::new();
        cancel_binding.insert("shortcut", zbus::zvariant::Value::new(self.cancel_shortcut.clone()));
        cancel_binding.insert("description", zbus::zvariant::Value::new("Cancel recording"));
        shortcuts.insert("cancel", cancel_binding);

        let _binding_handle = proxy.bind_shortcuts(&session_handle, shortcuts, "").await?;
        tracing::info!("Registered shortcuts");

        // TODO: Portal signal handling needs proper zbus signal subscription
        // For now, portal shortcuts registration is done but signal handling
        // needs to be implemented with proper zbus signal API
        tracing::warn!("Portal shortcuts registered but signal handling not yet implemented");
        tracing::warn!("Push-to-talk mode will work, but toggle shortcuts may not function");
        
        // Keep connection alive
        loop {
            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
        }
    }
}

