//! Persistent user settings, shared runtime state and the commands used by the
//! settings window.

use std::{fs, path::PathBuf, sync::Mutex};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_dialog::DialogExt;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub video_path: Option<String>,
    /// 24, 30, 60 or 0 (= unlimited)
    pub fps_limit: u32,
    /// 0.0 ..= 1.0
    pub volume: f64,
    pub muted: bool,
    pub pause_when_covered: bool,
    pub pause_on_battery: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            video_path: None,
            fps_limit: 30,
            volume: 0.5,
            muted: true,
            pause_when_covered: true,
            pause_on_battery: false,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeState {
    /// Every monitor is fully covered by some other window.
    pub covered: bool,
    pub on_battery: bool,
}

pub struct AppState {
    pub settings: Mutex<Settings>,
    pub runtime: Mutex<RuntimeState>,
}

impl AppState {
    pub fn new(settings: Settings) -> Self {
        Self {
            settings: Mutex::new(settings),
            runtime: Mutex::new(RuntimeState::default()),
        }
    }
}

fn settings_path(app: &AppHandle) -> Result<PathBuf, String> {
    app.path()
        .app_config_dir()
        .map(|d| d.join("settings.json"))
        .map_err(|e| e.to_string())
}

pub fn load(app: &AppHandle) -> Settings {
    settings_path(app)
        .ok()
        .and_then(|p| fs::read_to_string(p).ok())
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default()
}

fn save(app: &AppHandle, settings: &Settings) -> Result<(), String> {
    let path = settings_path(app)?;
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let json = serde_json::to_vec_pretty(settings).map_err(|e| e.to_string())?;
    fs::write(path, json).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_settings(state: State<'_, AppState>) -> Settings {
    state.settings.lock().unwrap().clone()
}

#[tauri::command]
pub fn get_runtime(state: State<'_, AppState>) -> RuntimeState {
    state.runtime.lock().unwrap().clone()
}

#[tauri::command]
pub fn set_settings(
    app: AppHandle,
    state: State<'_, AppState>,
    mut settings: Settings,
) -> Result<(), String> {
    settings.fps_limit = match settings.fps_limit {
        24 | 30 | 60 => settings.fps_limit,
        _ => 0,
    };
    settings.volume = settings.volume.clamp(0.0, 1.0);

    if let Some(path) = &settings.video_path {
        if !std::path::Path::new(path).is_file() {
            return Err(format!("File not found: {path}"));
        }
        // Only the chosen file is readable through the asset protocol.
        app.asset_protocol_scope()
            .allow_file(path)
            .map_err(|e| e.to_string())?;
    }

    save(&app, &settings)?;
    *state.settings.lock().unwrap() = settings.clone();
    app.emit("settings:changed", &settings)
        .map_err(|e| e.to_string())
}

/// Opens the native file picker. Runs on the async runtime because the
/// blocking dialog must not run on the main thread.
#[tauri::command]
pub async fn pick_video(app: AppHandle, window: tauri::WebviewWindow) -> Option<String> {
    let file = app
        .dialog()
        .file()
        .set_parent(&window)
        .add_filter(
            "Video",
            &[
                "mp4", "m4v", "mov", "webm", "mkv", "ogv", "ogg", "avi", "wmv", "flv", "mpg",
                "mpeg", "ts", "m2ts", "3gp",
            ],
        )
        .add_filter("All files", &["*"])
        .blocking_pick_file()?;
    file.into_path()
        .ok()
        .map(|p| p.to_string_lossy().into_owned())
}
