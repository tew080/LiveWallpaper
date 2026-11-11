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
};

export const FPS_OPTIONS = [
  { value: 24, label: "24" },
  { value: 30, label: "30" },
  { value: 60, label: "60" },
  { value: 0, label: "Unlimited" },
];

export const LOOP_OPTIONS = [
  { value: 0, label: "Seamless" },
  { value: 0.5, label: "0.5 s fade" },
  { value: 1, label: "1 s fade" },
  { value: 2, label: "2 s fade" },
];
