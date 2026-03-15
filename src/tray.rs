use muda::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tokio_util::sync::CancellationToken;
use tray_icon::{TrayIcon, TrayIconBuilder};
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::WindowId;

struct TrayHandler {
    ct: CancellationToken,
    _tray: TrayIcon,
    quit_id: muda::MenuId,
    dashboard_id: muda::MenuId,
    dashboard_url: String,
}

impl ApplicationHandler for TrayHandler {
    fn resumed(&mut self, _event_loop: &ActiveEventLoop) {}

    fn window_event(&mut self, _event_loop: &ActiveEventLoop, _id: WindowId, _event: WindowEvent) {}

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if self.ct.is_cancelled() {
            event_loop.exit();
            return;
        }

        while let Ok(event) = MenuEvent::receiver().try_recv() {
            if event.id() == &self.quit_id {
                tracing::info!("Quit selected from menu bar");
                self.ct.cancel();
                event_loop.exit();
                return;
            } else if event.id() == &self.dashboard_id {
                open_url(&self.dashboard_url);
            }
        }
    }
}

pub fn run(ct: CancellationToken, bind_addr: &str) -> Result<(), Box<dyn std::error::Error>> {
    let dashboard_url = format!("http://{}/api/v1/stats", bind_addr);

    // Build menu
    let menu = Menu::new();
    let status = MenuItem::new(
        format!("Stele — Running on {}", bind_addr),
        false,
        None,
    );
    let dashboard = MenuItem::new("Open Dashboard", true, None);
    let quit = MenuItem::new("Quit Stele", true, None);

    menu.append(&status)?;
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
        dashboard_url,
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
    let _ = std::process::Command::new("cmd").args(["/C", "start", url]).spawn();

    #[cfg(target_os = "linux")]
    let _ = std::process::Command::new("xdg-open").arg(url).spawn();
}
