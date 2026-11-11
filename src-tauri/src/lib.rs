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
    let context = tauri::generate_context!();

    // WebView2 flags, shared by every webview (wallpaper + settings) because
    // they are set through an environment variable. They depend on the
    // hardware options in settings.json, so they are read here, before the
    // first webview starts (changing them needs a restart).
    #[cfg(windows)]
    {
        let user = settings::load_early(&context.config().identifier);
        // SAFETY: runs first in `run()`, before any other thread is started.
        unsafe {
            std::env::set_var(
                "WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS",
                settings::webview_args(&user),
            );
        }
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
            settings::set_autostart,
            settings::restart_app,
            settings::open_graphics_settings
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
        .run(context)
        .expect("error while running tauri application");
}
