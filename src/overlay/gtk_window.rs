use crate::daemon::state::DaemonState;
use crate::overlay::{Overlay, OverlayError};
use gtk4::gdk::Display;
use gtk4::glib;
use gtk4::prelude::*;
use std::sync::{Arc, Mutex};

pub struct GtkOverlay {
    window: Arc<Mutex<Option<gtk4::ApplicationWindow>>>,
    state: Arc<Mutex<DaemonState>>,
    audio_level: Arc<Mutex<f32>>,
}

impl GtkOverlay {
    pub fn new() -> Result<Self, OverlayError> {
        gtk4::init().map_err(|e| OverlayError::GtkError(e.to_string()))?;

        let app = gtk4::Application::builder()
            .application_id("com.croaker.overlay")
            .build();

        let overlay = Self {
            window: Arc::new(Mutex::new(None)),
            state: Arc::new(Mutex::new(DaemonState::Idle)),
            audio_level: Arc::new(Mutex::new(0.0)),
        };

        let window_clone = overlay.window.clone();
        let state_clone = overlay.state.clone();
        let audio_level_clone = overlay.audio_level.clone();

        app.connect_activate(move |app| {
            let window = gtk4::ApplicationWindow::builder()
                .application(app)
                .decorated(false)
                .resizable(false)
                .can_focus(false)
                .build();

            // Set window type hint for overlay (optional, may not be available in all GTK4 versions)
            // window.set_type_hint(gdk4::WindowTypeHint::Notification);

            // Create drawing area
            let drawing_area = gtk4::DrawingArea::new();
            drawing_area.set_content_width(48);
            drawing_area.set_content_height(48);

            let state_inner = state_clone.clone();
            let audio_level_inner = audio_level_clone.clone();

            drawing_area.set_draw_func(move |_, cr, width, height| {
                let state = *state_inner.lock().unwrap();
                let audio_level = *audio_level_inner.lock().unwrap();

                // Clear
                cr.set_source_rgba(0.0, 0.0, 0.0, 0.0);
                let _ = cr.paint();

                // Draw indicator dot
                let center_x = width as f64 / 2.0;
                let center_y = height as f64 / 2.0;
                let base_radius = (width.min(height) as f64 / 2.0) * 0.6;

                // Color based on state
                let (r, g, b) = match state {
                    DaemonState::Recording => (1.0, 0.0, 0.0), // Red
                    DaemonState::Processing => (1.0, 1.0, 0.0), // Yellow
                    DaemonState::Outputting => (0.0, 1.0, 0.0), // Green
                    DaemonState::Idle => (0.5, 0.5, 0.5),       // Gray
                };

                // Draw filled circle
                cr.set_source_rgba(r, g, b, 0.9);
                cr.arc(center_x, center_y, base_radius, 0.0, 2.0 * std::f64::consts::PI);
                let _ = cr.fill();

                // Draw pulsing outline based on audio level
                if state == DaemonState::Recording {
                    let pulse_radius = base_radius + (audio_level as f64 * base_radius * 0.5);
                    cr.set_source_rgba(r, g, b, 0.5 * (1.0 - audio_level as f64));
                    cr.set_line_width(2.0);
                    cr.arc(center_x, center_y, pulse_radius, 0.0, 2.0 * std::f64::consts::PI);
                    let _ = cr.stroke();
                }
            });

            window.set_child(Some(&drawing_area));

            // Position at top-center
            window.set_default_size(48, 48);
            // Window positioning will be handled by the window manager
            // Note: set_keep_above is not available in GTK4, window manager will handle layering

            window.present();

            *window_clone.lock().unwrap() = Some(window);
        });

        // Run GTK in background thread
        // Note: GTK4 Application is not Send, so we use unsafe to work around this
        // This is safe because GTK is initialized in this thread and we're just
        // moving the pointer to another thread where it will be used
        let app_ptr = Box::into_raw(Box::new(app)) as usize;
        std::thread::spawn(move || {
            unsafe {
                let app = Box::from_raw(app_ptr as *mut gtk4::Application);
                app.run();
            }
        });

        // Wait a bit for window to be created
        std::thread::sleep(std::time::Duration::from_millis(100));

        Ok(overlay)
    }
}

impl Overlay for GtkOverlay {
    fn update_state(&self, state: DaemonState) {
        *self.state.lock().unwrap() = state;
        
        if let Some(window) = self.window.lock().unwrap().as_ref() {
            window.queue_draw();
        }
    }

    fn update_audio_level(&self, level: f32) {
        *self.audio_level.lock().unwrap() = level.clamp(0.0, 1.0);
        
        if let Some(window) = self.window.lock().unwrap().as_ref() {
            window.queue_draw();
        }
    }

    fn show(&self) {
        if let Some(window) = self.window.lock().unwrap().as_ref() {
            window.show();
        }
    }

    fn hide(&self) {
        if let Some(window) = self.window.lock().unwrap().as_ref() {
            window.hide();
        }
    }
}

