#[cfg_attr(not(feature = "desktop"), allow(dead_code))]
pub struct NotificationRequest {
    pub title: String,
    pub body: String,
    pub subtitle: Option<String>,
    pub sound: bool,
}

/// Pin the bundle identifier used by `mac-notification-sys` so it does NOT
/// run its default lookup path, which on macOS 13+ pops a "Choose Application"
/// dialog when asked to resolve its internal `"use_default"` sentinel name.
/// Must be called once at startup, before any `show()` call.
#[cfg(all(feature = "desktop", target_os = "macos"))]
pub fn init() {
    // Ignore the result: set_application uses a `Once` internally, so a
    // second call just returns `AlreadySet`. That's fine for idempotency.
    let _ = notify_rust::set_application("com.tasanakorn.stele.app");
}

#[cfg(not(all(feature = "desktop", target_os = "macos")))]
pub fn init() {}

#[cfg(feature = "desktop")]
pub fn show(req: &NotificationRequest) -> Result<(), String> {
    use notify_rust::Notification;
    let mut n = Notification::new();
    n.summary(&req.title).body(&req.body);
    #[cfg(target_os = "macos")]
    if let Some(sub) = &req.subtitle {
        n.subtitle(sub);
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = &req.subtitle; // suppress unused warning on non-mac desktop
    }
    if req.sound {
        n.sound_name("default");
    }
    n.show().map(|_| ()).map_err(|e| e.to_string())
}

#[cfg(not(feature = "desktop"))]
pub fn show(_req: &NotificationRequest) -> Result<(), String> {
    Err("notifications unavailable in headless build".to_string())
}
