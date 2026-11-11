export interface Settings {
  videoPath: string | null;
  /** 24, 30, 60 or 0 (= unlimited) */
  fpsLimit: number;
  /** 0..1 */
  volume: number;
  muted: boolean;
  pauseWhenCovered: boolean;
  pauseOnBattery: boolean;
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
};

export const FPS_OPTIONS = [
  { value: 24, label: "24" },
  { value: 30, label: "30" },
  { value: 60, label: "60" },
  { value: 0, label: "Unlimited" },
];
