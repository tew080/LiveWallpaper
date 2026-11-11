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
    /// Picture adjustments in percent (100 = unchanged), applied as a CSS filter.
    pub brightness: f64,
    pub contrast: f64,
    pub saturation: f64,
    /// Degrees, -180.0 ..= 180.0
    pub hue: f64,
    /// Pixels, 0.0 ..= 20.0
    pub blur: f64,
    /// 0.0 ..= 100.0
    pub vignette: f64,
    /// Playback speed, 0.25 ..= 2.0
    pub speed: f64,
    /// Loop transition in seconds: 0 = seamless hand-off, > 0 = crossfade
    pub loop_fade: f64,
    /// Picture adjustments, 0.0 ..= 100.0
    pub grayscale: f64,
    pub sepia: f64,
    /// Sharpening strength, 0.0 ..= 100.0
    pub sharpen: f64,
    /// Film grain, scanlines and tint strength, 0.0 ..= 100.0
    pub grain: f64,
    pub scanlines: f64,
    pub tint_color: String,
    pub tint_strength: f64,
    /// High-quality resampling (anti-aliasing when the video is scaled down).
    pub smooth_scaling: bool,
    /// "cover" (fill, crop), "contain" (fit, bars) or "fill" (stretch)
    pub fit: String,
    /// Hardware options. They are read when the app starts (see `webview_args`).
    pub gpu_acceleration: bool,
    pub hw_video_decode: bool,
    /// "compatible", "balanced" or "maximum"
    pub gpu_profile: String,
    /// "auto", "power" (power saving GPU) or "performance" (high performance GPU).
    /// Stored per program by Windows, see `apply_gpu_choice`.
    pub gpu_choice: String,
    /// FXAA edge anti-aliasing in the GPU shader.
    pub anti_aliasing: bool,
    /// 0.0 ..= 100.0
    pub aberration: f64,
    pub pixelate: f64,
    /// -100.0 (cool) ..= 100.0 (warm)
    pub temperature: f64,
    /// Internal render resolution, 50.0 ..= 150.0 (% of the screen). Above 100
    /// is supersampling, offered for dedicated GPUs.
    pub render_scale: f64,
    /// FSR-lite: render below screen size, then upscale (Catmull-Rom + contrast
    /// adaptive sharpening) in a second GPU pass.
    pub upscaler: bool,
    /// Sharpening strength of the upscaler, 0.0 ..= 100.0
    pub upscale_sharp: f64,
    /// FXAA strength, 0.0 ..= 200.0 (%)
    pub aa_amount: f64,
    /// Glow around bright areas, 0.0 ..= 100.0
    pub bloom: f64,
    /// Shows the original on the left half of the screen, to judge the effects.
    pub compare: bool,
    /// Horizontal position of the compare divider, 0.0 ..= 1.0
    pub compare_x: f64,
    /// Temporal anti-aliasing (2-frame) used with the upscaler
    pub taa: bool,
    /// Overall strength of the FSR-lite upscaler pass, 0.0 ..= 100.0
    pub fsr_strength: f64,
    /// Frame limit while on battery: 0 = unchanged
    pub battery_fps: u32,
    /// "auto", "en" or "th"
    pub language: String,
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
            brightness: 100.0,
            contrast: 100.0,
            saturation: 100.0,
            hue: 0.0,
            blur: 0.0,
            vignette: 0.0,
            speed: 1.0,
            loop_fade: 0.0,
            grayscale: 0.0,
            sepia: 0.0,
            sharpen: 0.0,
            grain: 0.0,
            scanlines: 0.0,
            tint_color: "#ffa65c".into(),
            tint_strength: 0.0,
            smooth_scaling: false,
            fit: "cover".into(),
            gpu_acceleration: true,
            hw_video_decode: true,
            gpu_profile: "maximum".into(),
            gpu_choice: "auto".into(),
            anti_aliasing: false,
            aberration: 0.0,
            pixelate: 0.0,
            temperature: 0.0,
            render_scale: 100.0,
            upscaler: false,
            upscale_sharp: 50.0,
            aa_amount: 100.0,
            bloom: 0.0,
            compare: false,
            compare_x: 0.5,
            taa: true,
            fsr_strength: 60.0,
            battery_fps: 0,
            language: "auto".into(),
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

/// Reads `settings.json` before Tauri exists. WebView2 takes its flags when the
/// first webview starts, which is before the app handle (and its path API) is
/// available, so the file is found the same way `app_config_dir` does on Windows:
/// `%APPDATA%\<identifier>`.
#[allow(dead_code)]
pub fn load_early(identifier: &str) -> Settings {
    let settings: Settings = std::env::var_os("APPDATA")
        .map(|d| PathBuf::from(d).join(identifier).join("settings.json"))
        .and_then(|p| fs::read_to_string(p).ok())
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default();
    // The WebView2 runtime lives in a versioned folder that changes when it
    // updates, so the GPU choice is written again on every start. This runs
    // before the webview starts, so the choice is already in place for it.
    if settings.gpu_choice != "auto" {
        apply_gpu_choice(&settings.gpu_choice);
    }
    settings
}

/// Command-line flags for WebView2, chosen from the hardware options.
///
/// * Autoplay is always allowed: a wallpaper can't be clicked.
/// * Occlusion tracking is always off. A wallpaper sits behind every window, so
///   Chromium would treat it as hidden and throttle it; pausing is done by the
///   app itself (see monitor.rs).
/// * "compatible" adds nothing else, "balanced" adds GPU rasterisation and
///   zero-copy uploads, "maximum" also ignores Chromium's GPU driver blocklist.
#[allow(dead_code)]
pub fn webview_args(s: &Settings) -> String {
    let mut args: Vec<&str> = vec!["--autoplay-policy=no-user-gesture-required"];
    let mut enable: Vec<&str> = Vec::new();
    // Occlusion tracking off (see above); the rest are browser services a
    // wallpaper never uses.
    let disable: Vec<&str> = vec![
        "CalculateNativeWinOcclusion",
        "Translate",
        "MediaRouter",
        "OptimizationHints",
        "AutofillServerCommunication",
        "CertificateTransparencyComponentUpdater",
    ];

    if !s.gpu_acceleration {
        args.push("--disable-gpu");
    } else {
        if s.hw_video_decode {
            enable.push("PlatformHEVCDecoderSupport"); // HEVC needs GPU/OS support
        } else {
            args.push("--disable-accelerated-video-decode");
        }
        if s.gpu_profile != "compatible" {
            args.push("--enable-gpu-rasterization");
            args.push("--enable-zero-copy");
        }
        if s.gpu_profile == "maximum" {
            args.push("--ignore-gpu-blocklist");
        }
    }

    if s.gpu_acceleration && s.gpu_choice == "performance" {
        // Chromium's own request for the high performance adapter, on top of
        // the Windows per-program preference. Ignored if a build doesn't know it.
        args.push("--force_high_performance_gpu");
    }
    args.push("--disable-background-networking");
    args.push("--disable-sync");
    args.push("--disable-default-apps");
    args.push("--disable-component-update");
    args.push("--disable-domain-reliability");
    args.push("--disable-client-side-phishing-detection");
    args.push("--disable-background-timer-throttling");
    args.push("--disable-renderer-backgrounding");
    args.push("--disable-backgrounding-occluded-windows");

    let enable = format!("--enable-features={}", enable.join(","));
    let disable = format!("--disable-features={}", disable.join(","));
    if enable != "--enable-features=" {
        args.push(&enable);
    }
    args.push(&disable);
    args.join(" ")
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
    settings.brightness = settings.brightness.clamp(20.0, 200.0);
    settings.contrast = settings.contrast.clamp(50.0, 200.0);
    settings.saturation = settings.saturation.clamp(0.0, 200.0);
    settings.hue = settings.hue.clamp(-180.0, 180.0);
    settings.blur = settings.blur.clamp(0.0, 20.0);
    settings.vignette = settings.vignette.clamp(0.0, 100.0);
    settings.speed = settings.speed.clamp(0.25, 2.0);
    settings.loop_fade = settings.loop_fade.clamp(0.0, 3.0);
    settings.grayscale = settings.grayscale.clamp(0.0, 100.0);
    settings.sepia = settings.sepia.clamp(0.0, 100.0);
    settings.sharpen = settings.sharpen.clamp(0.0, 100.0);
    settings.grain = settings.grain.clamp(0.0, 100.0);
    settings.scanlines = settings.scanlines.clamp(0.0, 100.0);
    settings.tint_strength = settings.tint_strength.clamp(0.0, 100.0);
    settings.aberration = settings.aberration.clamp(0.0, 100.0);
    settings.pixelate = settings.pixelate.clamp(0.0, 100.0);
    settings.temperature = settings.temperature.clamp(-100.0, 100.0);
    settings.render_scale = settings.render_scale.clamp(50.0, 150.0);
    settings.upscale_sharp = settings.upscale_sharp.clamp(0.0, 100.0);
    settings.aa_amount = settings.aa_amount.clamp(0.0, 200.0);
    settings.bloom = settings.bloom.clamp(0.0, 100.0);
    settings.compare_x = settings.compare_x.clamp(0.0, 1.0);
    settings.fsr_strength = settings.fsr_strength.clamp(0.0, 100.0);
    settings.battery_fps = match settings.battery_fps {
        15 | 24 | 30 => settings.battery_fps,
        _ => 0,
    };
    if !matches!(settings.gpu_choice.as_str(), "auto" | "power" | "performance") {
        settings.gpu_choice = Settings::default().gpu_choice;
    }
    if !matches!(settings.language.as_str(), "auto" | "en" | "th") {
        settings.language = Settings::default().language;
    }
    let hex = settings.tint_color.strip_prefix('#').unwrap_or("");
    if hex.len() != 6 || !hex.chars().all(|c| c.is_ascii_hexdigit()) {
        settings.tint_color = Settings::default().tint_color;
    }
    if !matches!(settings.fit.as_str(), "cover" | "contain" | "fill") {
        settings.fit = Settings::default().fit;
    }
    if !matches!(
        settings.gpu_profile.as_str(),
        "compatible" | "balanced" | "maximum"
    ) {
        settings.gpu_profile = Settings::default().gpu_profile;
    }

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
    let previous = std::mem::replace(&mut *state.settings.lock().unwrap(), settings.clone());
    if previous.gpu_choice != settings.gpu_choice {
        apply_gpu_choice(&settings.gpu_choice);
    }
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

#[tauri::command]
pub fn get_autostart(app: AppHandle) -> bool {
    #[cfg(windows)]
    {
        use tauri_plugin_autostart::ManagerExt;
        app.autolaunch().is_enabled().unwrap_or(false)
    }
    #[cfg(not(windows))]
    {
        let _ = app;
        false
    }
}

/// Returns the state actually stored in the registry after the change.
#[tauri::command]
pub fn set_autostart(app: AppHandle, enabled: bool) -> Result<bool, String> {
    #[cfg(windows)]
    {
        use tauri_plugin_autostart::ManagerExt;
        if enabled && cfg!(debug_assertions) {
            // A dev build would register target\debug\*.exe, which can't work at login.
            return Err("Start with Windows only works in an installed (release) build.".into());
        }
        let manager = app.autolaunch();
        let result = if enabled { manager.enable() } else { manager.disable() };
        result.map_err(|e| e.to_string())?;
        manager.is_enabled().map_err(|e| e.to_string())
    }
    #[cfg(not(windows))]
    {
        let _ = (app, enabled);
        Err("Start with Windows is only available on Windows.".into())
    }
}

/// Restarts the app, e.g. to apply hardware options.
#[tauri::command]
pub fn restart_app(app: AppHandle) {
    app.restart();
}

/// Opens Windows "Graphics" settings, where a GPU can be chosen per program.
#[tauri::command]
pub fn open_graphics_settings() -> Result<(), String> {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        std::process::Command::new("explorer.exe")
            .arg("ms-settings:display-advanced-graphics")
            .creation_flags(0x0800_0000) // CREATE_NO_WINDOW
            .spawn()
            .map(|_| ())
            .map_err(|e| e.to_string())
    }
    #[cfg(not(windows))]
    {
        Err("Graphics settings are only available on Windows.".into())
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GpuInfo {
    pub name: String,
    /// "integrated" or "discrete" (a guess from the name)
    pub kind: String,
}

/// Lists the graphics adapters Windows knows about.
#[tauri::command]
pub async fn list_gpus() -> Result<Vec<GpuInfo>, String> {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        let out = std::process::Command::new("powershell.exe")
            .args([
                "-NoProfile",
                "-NonInteractive",
                "-Command",
                "Get-CimInstance Win32_VideoController | Select-Object -ExpandProperty Name | ConvertTo-Json -Compress",
            ])
            .creation_flags(0x0800_0000) // CREATE_NO_WINDOW
            .output()
            .map_err(|e| e.to_string())?;
        let text = String::from_utf8_lossy(&out.stdout);
        let value: serde_json::Value =
            serde_json::from_str(text.trim_start_matches('\u{feff}').trim()).map_err(|e| e.to_string())?;
        let names: Vec<String> = match value {
            serde_json::Value::String(s) => vec![s],
            serde_json::Value::Array(a) => a
                .into_iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect(),
            _ => Vec::new(),
        };
        Ok(names
            .into_iter()
            // virtual adapters (remote desktop, basic display driver)
            .filter(|n| {
                let l = n.to_lowercase();
                !(l.contains("basic render") || l.contains("remote") || l.contains("virtual"))
            })
            .map(|name| {
                let l = name.to_lowercase();
                let discrete = l.contains("nvidia")
                    || l.contains("geforce")
                    || l.contains("quadro")
                    || l.contains(" arc ")
                    || l.contains("radeon rx")
                    || l.contains("radeon pro");
                GpuInfo {
                    kind: if discrete { "discrete" } else { "integrated" }.into(),
                    name,
                }
            })
            .collect())
    }
    #[cfg(not(windows))]
    {
        Err("Listing graphics cards is only available on Windows.".into())
    }
}

/// Writes (or removes) Windows' per-program GPU preference for the WebView2
/// runtime, which is the process that draws the wallpaper, and for this app.
/// This is the same value the Windows "Graphics settings" page stores:
/// `GpuPreference=1` is the power saving GPU, `2` the high performance GPU.
fn apply_gpu_choice(choice: &str) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const KEY: &str = r"HKCU\Software\Microsoft\DirectX\UserGpuPreferences";
        let mut exes: Vec<PathBuf> = Vec::new();
        if let Ok(me) = std::env::current_exe() {
            exes.push(me);
        }
        for var in ["ProgramFiles(x86)", "ProgramFiles"] {
            let Some(root) = std::env::var_os(var) else { continue };
            let dir = PathBuf::from(root).join(r"Microsoft\EdgeWebView\Application");
            let Ok(entries) = fs::read_dir(dir) else { continue };
            for e in entries.flatten() {
                let exe = e.path().join("msedgewebview2.exe");
                if exe.is_file() {
                    exes.push(exe);
                }
            }
        }
        for exe in exes {
            let path = exe.to_string_lossy().into_owned();
            let mut cmd = std::process::Command::new("reg.exe");
            match choice {
                "power" => cmd.args(["add", KEY, "/v", path.as_str(), "/t", "REG_SZ", "/d", "GpuPreference=1;", "/f"]),
                "performance" => cmd.args(["add", KEY, "/v", path.as_str(), "/t", "REG_SZ", "/d", "GpuPreference=2;", "/f"]),
                _ => cmd.args(["delete", KEY, "/v", path.as_str(), "/f"]),
            };
            let _ = cmd.creation_flags(0x0800_0000).output();
        }
    }
    #[cfg(not(windows))]
    {
        let _ = choice;
    }
}
