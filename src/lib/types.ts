export interface Settings {
  videoPath: string | null;
  /** 24, 30, 60 or 0 (= unlimited) */
  fpsLimit: number;
  /** 0..1 */
  volume: number;
  muted: boolean;
  pauseWhenCovered: boolean;
  pauseOnBattery: boolean;
  /** Picture adjustments, applied on the GPU. 100 = unchanged. */
  brightness: number;
  contrast: number;
  saturation: number;
  /** degrees, -180..180 */
  hue: number;
  /** px, 0..20 */
  blur: number;
  /** 0..100 */
  vignette: number;
  /** playback speed, 0.25..2 */
  speed: number;
  /** seconds, 0 = seamless hand-off, > 0 = crossfade */
  loopFade: number;
  /** 0..100 */
  grayscale: number;
  sepia: number;
  /** Sharpening strength, 0..100 */
  sharpen: number;
  /** Film grain, scanlines and tint strength, 0..100 */
  grain: number;
  scanlines: number;
  tintColor: string;
  tintStrength: number;
  /** High-quality resampling through a canvas (less aliasing when scaled down). */
  smoothScaling: boolean;
  fit: "cover" | "contain" | "fill";
  /** Hardware options, applied after a restart. */
  gpuAcceleration: boolean;
  hwVideoDecode: boolean;
  gpuProfile: "compatible" | "balanced" | "maximum";
  /** Which GPU Windows should use for the wallpaper engine (after a restart). */
  gpuChoice: "auto" | "power" | "performance";
  /** Edge anti-aliasing (FXAA) in the GPU shader. */
  antiAliasing: boolean;
  /** Chromatic aberration, pixelate: 0..100. Colour temperature: -100 (cool)..100 (warm). */
  aberration: number;
  pixelate: number;
  temperature: number;
  /** Internal render resolution, 50..150 (% of the screen). Above 100 = supersampling. */
  renderScale: number;
  /** FSR-lite: render below screen size, then upscale in a second GPU pass. */
  upscaler: boolean;
  /** Upscaler sharpening, 0..100. */
  upscaleSharp: number;
  /** FXAA strength, 0..200 (%). */
  aaAmount: number;
  /** Glow around bright areas, 0..100. */
  bloom: number;
  /** Original on the left half, processed on the right, to judge the effects. */
  compare: boolean;
  /** Frame limit used while on battery: 0 = same as above. */
  batteryFps: number;
  /** "auto" follows the system language. */
  language: "auto" | "en" | "th";
}

export interface RuntimeState {
  covered: boolean;
  onBattery: boolean;
}

// Keep in sync with `Settings::default()` in src-tauri/src/settings.rs
export const DEFAULT_SETTINGS: Settings = {
  videoPath: null,
  fpsLimit: 30,
  volume: 0.5,
  muted: true,
  pauseWhenCovered: true,
  pauseOnBattery: false,
  brightness: 100,
  contrast: 100,
  saturation: 100,
  hue: 0,
  blur: 0,
  vignette: 0,
  speed: 1,
  loopFade: 0,
  grayscale: 0,
  sepia: 0,
  sharpen: 0,
  grain: 0,
  scanlines: 0,
  tintColor: "#ffa65c",
  tintStrength: 0,
  smoothScaling: false,
  fit: "cover",
  gpuAcceleration: true,
  hwVideoDecode: true,
  gpuProfile: "maximum",
  gpuChoice: "auto",
  antiAliasing: false,
  aberration: 0,
  pixelate: 0,
  temperature: 0,
  renderScale: 100,
  upscaler: false,
  upscaleSharp: 50,
  aaAmount: 150,
  bloom: 0,
  compare: false,
  batteryFps: 0,
  language: "auto",
};

// Option labels are looked up in i18n.ts by their `key`.
export const FPS_OPTIONS = [
  { value: 24, key: "24" },
  { value: 30, key: "30" },
  { value: 60, key: "60" },
  { value: 0, key: "opt.unlimited" },
];

export const BATTERY_FPS_OPTIONS = [
  { value: 0, key: "opt.noChange" },
  { value: 15, key: "15" },
  { value: 24, key: "24" },
  { value: 30, key: "30" },
];

export const LOOP_OPTIONS = [
  { value: 0, key: "opt.seamless" },
  { value: 0.5, key: "opt.fade05" },
  { value: 1, key: "opt.fade1" },
  { value: 2, key: "opt.fade2" },
];

export const FIT_OPTIONS = [
  { value: "cover", key: "opt.fill" },
  { value: "contain", key: "opt.fit" },
  { value: "fill", key: "opt.stretch" },
];

export const PROFILE_OPTIONS = [
  { value: "compatible", key: "opt.compatible" },
  { value: "balanced", key: "opt.balanced" },
  { value: "maximum", key: "opt.maximum" },
];

export const LANGUAGE_OPTIONS = [
  { value: "auto", key: "opt.auto" },
  { value: "en", key: "English" },
  { value: "th", key: "ไทย" },
];

/** One-click looks for the Effects tab. Missing keys fall back to the defaults. */
export const PRESETS: { id: string; values: Partial<Settings> }[] = [
  { id: "cinematic", values: { vignette: 45, grain: 18, contrast: 112, saturation: 90, temperature: 12 } },
  { id: "retro", values: { scanlines: 45, aberration: 22, vignette: 35, grain: 12, saturation: 115 } },
  { id: "noir", values: { grayscale: 100, contrast: 125, vignette: 50, grain: 25 } },
  { id: "warm", values: { temperature: 40, tintColor: "#ffa65c", tintStrength: 14, vignette: 20, saturation: 108 } },
  { id: "crisp", values: { antiAliasing: true, aaAmount: 160, sharpen: 40, contrast: 105 } },
  { id: "glow", values: { bloom: 45, saturation: 110, contrast: 105, vignette: 15 } },
  { id: "dream", values: { blur: 1.5, brightness: 108, saturation: 120, temperature: -12, vignette: 25 } },
];

/** Presets: "Eco" and "Quality" for the Performance tab. */
export const POWER_MODES: { id: string; values: Partial<Settings> }[] = [
  {
    id: "eco",
    values: { fpsLimit: 24, renderScale: 75, batteryFps: 15, pauseWhenCovered: true, pauseOnBattery: false },
  },
  { id: "balanced", values: { fpsLimit: 30, renderScale: 100, batteryFps: 24, pauseWhenCovered: true } },
  { id: "quality", values: { fpsLimit: 60, renderScale: 100, batteryFps: 0, pauseWhenCovered: true } },
];
