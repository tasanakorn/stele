use crate::settings;
use crate::BindState;
use muda::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tray_icon::{TrayIcon, TrayIconBuilder};
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::WindowId;

struct TrayHandler {
    ct: tokio_util::sync::CancellationToken,
    _tray: TrayIcon,
    quit_id: muda::MenuId,
    dashboard_id: muda::MenuId,
    settings_id: muda::MenuId,
    dashboard_url: String,
    status_item: MenuItem,
    bind_state: Arc<BindState>,
    db_path: String,
    port: String,
    settings_child: Option<std::process::Child>,
    last_poll: Instant,
    last_known_ip: String,
}

impl ApplicationHandler for TrayHandler {
    fn resumed(&mut self, _event_loop: &ActiveEventLoop) {}

    fn window_event(
        &mut self,
        _event_loop: &ActiveEventLoop,
        _id: WindowId,
        _event: WindowEvent,
    ) {
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if self.ct.is_cancelled() {
            self.cleanup_child();
            event_loop.exit();
            return;
        }

        while let Ok(event) = MenuEvent::receiver().try_recv() {
            if event.id() == &self.quit_id {
                tracing::info!("Quit selected from menu bar");
                self.cleanup_child();
                self.ct.cancel();
                event_loop.exit();
                return;
            } else if event.id() == &self.dashboard_id {
                open_url(&self.dashboard_url);
            } else if event.id() == &self.settings_id {
                self.open_settings();
            }
        }

        // Poll config.toml while settings subprocess is alive
        if self.settings_child.is_some() {
            self.poll_settings();
            event_loop.set_control_flow(ControlFlow::WaitUntil(
                Instant::now() + Duration::from_millis(500),
            ));
        } else {
            event_loop.set_control_flow(ControlFlow::Wait);
        }
    }
}

impl TrayHandler {
    fn open_settings(&mut self) {
        if self.settings_child.is_some() {
            return; // already open
        }

        let exe = match std::env::current_exe() {
            Ok(p) => p,
            Err(e) => {
                tracing::error!("Failed to get current exe path: {e}");
                return;
            }
        };

        match std::process::Command::new(exe)
            .arg("--settings")
            .arg("--db")
            .arg(&self.db_path)
            .spawn()
        {
            Ok(child) => {
                self.settings_child = Some(child);
                self.last_poll = Instant::now();
            }
            Err(e) => {
                tracing::error!("Failed to spawn settings dialog: {e}");
            }
        }
    }

    fn poll_settings(&mut self) {
        let child = match self.settings_child.as_mut() {
            Some(c) => c,
            None => return,
        };

        // Check if subprocess exited
        match child.try_wait() {
            Ok(Some(_)) => {
                self.settings_child = None;
                // Do one final poll
                self.check_config_change();
                return;
            }
            Ok(None) => {} // still running
            Err(e) => {
                tracing::error!("Error checking settings subprocess: {e}");
                self.settings_child = None;
                return;
            }
        }

        if self.last_poll.elapsed() >= Duration::from_millis(500) {
            self.check_config_change();
            self.last_poll = Instant::now();
        }
    }

    fn check_config_change(&mut self) {
        let config_path = settings::settings_path(&self.db_path);
        let saved = settings::load_settings(&config_path);
        let new_ip = saved.server.bind_ip;

        if new_ip != self.last_known_ip {
            let new_addr = format!("{}:{}", new_ip, self.port);
            tracing::info!("Settings changed: bind address -> {new_addr}");

            // Update bind state and signal rebind
            {
                let mut addr = self.bind_state.bind_addr.write().unwrap();
                *addr = new_addr.clone();
            }
            self.bind_state.rebind_signal.notify_one();

            // Update UI
            self.status_item
                .set_text(format!("Stele \u{2014} Running on {new_addr}"));
            self.dashboard_url = crate::dashboard_url(&new_addr);
            self.last_known_ip = new_ip;
        }
    }

    fn cleanup_child(&mut self) {
        if let Some(mut child) = self.settings_child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

pub fn run(
    ct: tokio_util::sync::CancellationToken,
    bind_addr: &str,
    bind_state: Arc<BindState>,
    db_path: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let dashboard_url = crate::dashboard_url(bind_addr);

    // Extract IP and port from bind address
    let (current_ip, port) = bind_addr
        .rsplit_once(':')
        .map(|(ip, port)| (ip.to_string(), port.to_string()))
        .unwrap_or_else(|| (bind_addr.to_string(), "3100".to_string()));

    // Build menu
    let menu = Menu::new();
    let status = MenuItem::new(
        format!("Stele \u{2014} Running on {}", bind_addr),
        false,
        None,
    );
    let settings_item = MenuItem::new("Settings\u{2026}", true, None);
    let dashboard = MenuItem::new("Open Dashboard", true, None);
    let quit = MenuItem::new("Quit Stele", true, None);

    menu.append(&status)?;
    menu.append(&PredefinedMenuItem::separator())?;
    menu.append(&settings_item)?;
    menu.append(&PredefinedMenuItem::separator())?;
    menu.append(&dashboard)?;
    menu.append(&PredefinedMenuItem::separator())?;
    menu.append(&quit)?;

    // Load icon
    let icon_bytes = include_bytes!("../assets/icon.png");
    let img = image::load_from_memory(icon_bytes)?.into_rgba8();
    let (w, h) = img.dimensions();
    let icon = tray_icon::Icon::from_rgba(img.into_raw(), w, h)?;

    let tray = TrayIconBuilder::new()
        .with_menu(Box::new(menu))
        .with_icon(icon)
        .with_icon_as_template(true)
        .with_tooltip("Stele")
        .build()?;

    let mut handler = TrayHandler {
        ct,
        _tray: tray,
        quit_id: quit.id().clone(),
        dashboard_id: dashboard.id().clone(),
        settings_id: settings_item.id().clone(),
        dashboard_url,
        status_item: status,
        bind_state,
        db_path: db_path.to_string(),
        port,
        settings_child: None,
        last_poll: Instant::now(),
        last_known_ip: current_ip,
    };

    let event_loop = EventLoop::new()?;
    event_loop.set_control_flow(ControlFlow::Wait);
    event_loop.run_app(&mut handler)?;

    Ok(())
}

fn open_url(url: &str) {
    #[cfg(target_os = "macos")]
    let _ = std::process::Command::new("open").arg(url).spawn();

    #[cfg(target_os = "windows")]
    let _ = std::process::Command::new("cmd")
        .args(["/C", "start", url])
        .spawn();

    #[cfg(target_os = "linux")]
    let _ = std::process::Command::new("xdg-open").arg(url).spawn();
}
