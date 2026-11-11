mod settings;
mod wallpaper;
#[cfg(windows)]
mod monitor;

use settings::AppState;
use tauri::{
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
    AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder,
};

// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

fn open_settings(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("settings") {
        let _ = w.show();
        let _ = w.set_focus();
        return;
    }
    let _ = WebviewWindowBuilder::new(app, "settings", WebviewUrl::App("settings".into()))
        .title("LiveWallpaper settings")
        .inner_size(480.0, 760.0)
        .resizable(false)
        .build();
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // WebView2 flags, shared by every webview (wallpaper + settings) because
    // they are set through an environment variable.
    // * Wallpapers can't be clicked, so audio may autoplay without a gesture.
    // * HEVC hardware decoding is best-effort (needs GPU/OS support).
    // * Use the GPU for rasterising and zero-copy uploads, even on drivers that
    //   Chromium has on its blocklist (remove `--ignore-gpu-blocklist` if a
    //   driver shows glitches).
    // * A wallpaper sits behind every window, so Chromium would treat it as
    //   occluded and throttle or stop it. Pausing is done by the app itself
    //   (see monitor.rs), so these keep it running smoothly otherwise.
    #[cfg(windows)]
    // SAFETY: runs first in `run()`, before any other thread is started.
    unsafe {
        std::env::set_var(
            "WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS",
            [
                "--autoplay-policy=no-user-gesture-required",
                "--enable-features=PlatformHEVCDecoderSupport",
                "--disable-features=CalculateNativeWinOcclusion",
                "--ignore-gpu-blocklist",
                "--enable-gpu-rasterization",
                "--enable-zero-copy",
                "--disable-background-timer-throttling",
                "--disable-renderer-backgrounding",
                "--disable-backgrounding-occluded-windows",
            ]
            .join(" "),
        );
    }

    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init());

    // "Start with Windows" (HKCU Run key, no admin rights needed).
    #[cfg(windows)]
    let builder = builder.plugin(tauri_plugin_autostart::init(
        tauri_plugin_autostart::MacosLauncher::LaunchAgent,
        None,
    ));

    builder
        .invoke_handler(tauri::generate_handler![
            greet,
            settings::get_settings,
            settings::set_settings,
            settings::get_runtime,
            settings::pick_video,
            settings::get_autostart,
            settings::set_autostart
        ])
        .setup(|app| {
            let handle = app.handle().clone();

            let initial = settings::load(&handle);
            if let Some(path) = &initial.video_path {
                let _ = app.asset_protocol_scope().allow_file(path);
            }
            app.manage(AppState::new(initial));

            let window = app
                .get_webview_window("main")
                .ok_or("window `main` not found")?;

            // Embed the webview behind the desktop icons (Windows only). This
            // retries until Explorer is ready (e.g. when started at login) and
            // shows the window once it is attached.
            #[cfg(windows)]
            wallpaper::attach_in_background(
                window.clone(),
                app.path()
                    .app_log_dir()
                    .ok()
                    .map(|d| d.join("wallpaper-attach.log")),
            );
            #[cfg(not(windows))]
            {
                window.show()?;
            }

            // Tray menu: settings, pause/resume, quit.
            let settings_item = MenuItem::with_id(app, "settings", "Settings…", true, None::<&str>)?;
            let toggle = MenuItem::with_id(app, "toggle", "Pause / Resume", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&settings_item, &toggle, &quit])?;

            let mut tray = TrayIconBuilder::new()
                .tooltip("LiveWallpaper")
                .menu(&menu)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "settings" => {
                        let app = app.clone();
                        // Build the window off the main thread to avoid a deadlock on Windows.
                        std::thread::spawn(move || open_settings(&app));
                    }
                    "toggle" => {
                        let _ = app.emit("wallpaper:toggle", ());
                    }
                    "quit" => app.exit(0),
                    _ => {}
                });
            if let Some(icon) = app.default_window_icon() {
                tray = tray.icon(icon.clone());
            }
            tray.build(app)?;

            #[cfg(windows)]
            monitor::spawn(handle);

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
