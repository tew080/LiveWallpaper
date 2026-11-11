<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { DEFAULT_SETTINGS, FPS_OPTIONS, LOOP_OPTIONS, type Settings } from "$lib/types";

  const TABS = [
    { id: "wallpaper", label: "Wallpaper" },
    { id: "picture", label: "Picture" },
    { id: "performance", label: "Performance" },
    { id: "general", label: "General" },
  ] as const;
  type Tab = (typeof TABS)[number]["id"];

  const PICTURE_KEYS = ["brightness", "contrast", "saturation", "hue", "blur", "vignette"] as const;
  type NumKey = (typeof PICTURE_KEYS)[number] | "volume" | "speed";

  let tab = $state<Tab>("wallpaper");
  let s = $state<Settings>({ ...DEFAULT_SETTINGS });
  let status = $state<{ ok: boolean; message?: string } | null>(null);
  let support = $state<{ name: string; playback: string; hardware: string }[]>([]);
  let gpu = $state<string | null>(null);
  let timer: ReturnType<typeof setTimeout> | undefined;
  let autostart = $state(false);
  let autostartError = $state<string | null>(null);

  const fileName = $derived(s.videoPath?.split(/[\\/]/).pop() ?? null);

  // name, MIME type with codec, is it a video codec
  const probes: [string, string, boolean][] = [
    ["H.264 video", 'video/mp4; codecs="avc1.640028"', true],
    ["H.265 / HEVC video", 'video/mp4; codecs="hvc1.1.6.L93.B0"', true],
    ["VP8 video", 'video/webm; codecs="vp8"', true],
    ["VP9 video", 'video/webm; codecs="vp09.00.10.08"', true],
    ["AV1 video", 'video/mp4; codecs="av01.0.08M.08"', true],
    ["AAC audio", 'audio/mp4; codecs="mp4a.40.2"', false],
    ["Opus audio", 'audio/webm; codecs="opus"', false],
    ["Vorbis audio", 'audio/ogg; codecs="vorbis"', false],
    ["MP3 audio", "audio/mpeg", false],
    ["FLAC audio", "audio/flac", false],
  ];

  function save(immediate = false) {
    clearTimeout(timer);
    const push = () =>
      invoke("set_settings", { settings: $state.snapshot(s) }).catch(
        (e) => (status = { ok: false, message: String(e) }),
      );
    if (immediate) void push();
    else timer = setTimeout(push, 150);
  }

  async function browse() {
    const path = await invoke<string | null>("pick_video");
    if (path) {
      s.videoPath = path;
      status = null;
      save(true);
    }
  }

  function clearVideo() {
    s.videoPath = null;
    status = null;
    save(true);
  }

  function resetPicture() {
    for (const k of PICTURE_KEYS) s[k] = DEFAULT_SETTINGS[k];
    save(true);
  }

  async function setAutostart(input: HTMLInputElement) {
    try {
      autostart = await invoke<boolean>("set_autostart", { enabled: input.checked });
      autostartError = null;
    } catch (err) {
      input.checked = autostart; // revert the switch to what Windows actually has
      autostartError = String(err);
    }
  }

  function onTabKey(e: KeyboardEvent) {
    const i = TABS.findIndex((t) => t.id === tab);
    let n = i;
    if (e.key === "ArrowRight") n = (i + 1) % TABS.length;
    else if (e.key === "ArrowLeft") n = (i - 1 + TABS.length) % TABS.length;
    else return;
    e.preventDefault();
    tab = TABS[n].id;
    queueMicrotask(() => document.getElementById(`tab-${tab}`)?.focus());
  }

  const fmt = (v: number) => String(Math.round(v * 100) / 100);

  function gpuName(): string | null {
    try {
      const gl = document.createElement("canvas").getContext("webgl");
      const ext = gl?.getExtension("WEBGL_debug_renderer_info");
      const name = gl && ext ? String(gl.getParameter(ext.UNMASKED_RENDERER_WEBGL)) : null;
      gl?.getExtension("WEBGL_lose_context")?.loseContext();
      return name;
    } catch {
      return null;
    }
  }

  onMount(() => {
    const probe = document.createElement("video");
    support = probes.map(([name, type, video]) => {
      const r = probe.canPlayType(type);
      return {
        name,
        playback: r === "probably" ? "Yes" : r === "maybe" ? "Maybe" : "No",
        hardware: video ? "…" : "–",
      };
    });
    gpu = gpuName();

    // The engine reports "power efficient" for formats it decodes in hardware.
    probes.forEach(async ([, type, video], i) => {
      if (!video || !navigator.mediaCapabilities) return;
      let hw = "?";
      try {
        const info = await navigator.mediaCapabilities.decodingInfo({
          type: "file",
          video: { contentType: type, width: 1920, height: 1080, bitrate: 10_000_000, framerate: 30 },
        });
        hw = !info.supported ? "–" : info.powerEfficient ? "Yes" : "No";
      } catch {
        /* keep "?" */
      }
      support[i].hardware = hw;
    });

    if (!("__TAURI_INTERNALS__" in window)) return;

    let disposed = false;
    let off: (() => void) | undefined;

    invoke<Settings>("get_settings").then((v) => (s = v));
    invoke<boolean>("get_autostart").then((v) => (autostart = v));
    listen<{ ok: boolean; message?: string }>("wallpaper:status", (e) => {
      status = e.payload.ok && !e.payload.message ? null : e.payload;
    }).then((fn) => {
      if (disposed) fn();
      else off = fn;
    });

    return () => {
      disposed = true;
      off?.();
      clearTimeout(timer);
    };
  });
</script>

<svelte:head>
  <title>LiveWallpaper settings</title>
</svelte:head>

{#snippet toggle(
  id: string,
  label: string,
  desc: string,
  checked: boolean,
  change: (value: boolean, el: HTMLInputElement) => void,
)}
  <div class="row">
    <div class="lbl">
      <label class="t" for={id}>{label}</label>
      {#if desc}<div class="d">{desc}</div>{/if}
    </div>
    <span class="switch">
      <input
        {id}
        type="checkbox"
        role="switch"
        {checked}
        onchange={(e) => change(e.currentTarget.checked, e.currentTarget)}
      />
      <span class="track"></span>
    </span>
  </div>
{/snippet}

{#snippet slider(
  key: NumKey,
  label: string,
  min: number,
  max: number,
  step: number,
  unit: string,
  mult = 1,
  disabled = false,
)}
  {@const v = s[key] * mult}
  <div class="srow">
    <div class="head">
      <label for="r-{key}">{label}</label>
      <output for="r-{key}">{fmt(v)}{unit}</output>
    </div>
    <input
      id="r-{key}"
      type="range"
      {min}
      {max}
      {step}
      value={v}
      {disabled}
      style:--pct="{((v - min) / (max - min)) * 100}%"
      oninput={(e) => {
        s[key] = Number(e.currentTarget.value) / mult;
        save();
      }}
      ondblclick={() => {
        s[key] = DEFAULT_SETTINGS[key];
        save(true);
      }}
    />
  </div>
{/snippet}

{#snippet segmented(
  name: string,
  label: string,
  options: { value: number; label: string }[],
  current: number,
  pick: (value: number) => void,
)}
  <div class="seg" role="radiogroup" aria-label={label}>
    {#each options as o (o.value)}
      <label>
        <input type="radio" {name} value={o.value} checked={current === o.value} onchange={() => pick(o.value)} />
        <span>{o.label}</span>
      </label>
    {/each}
  </div>
{/snippet}

<div class="app">
  <header>
    <h1>LiveWallpaper</h1>
    <div class="tabs" role="tablist" aria-label="Settings sections">
      {#each TABS as t (t.id)}
        <button
          role="tab"
          id="tab-{t.id}"
          aria-selected={tab === t.id}
          aria-controls="panel"
          tabindex={tab === t.id ? 0 : -1}
          class:active={tab === t.id}
          onclick={() => (tab = t.id)}
          onkeydown={onTabKey}
        >
          {t.label}
        </button>
      {/each}
    </div>
  </header>

  <div class="panel" id="panel" role="tabpanel" aria-labelledby="tab-{tab}">
    {#if tab === "wallpaper"}
      <h2 class="cap">Video</h2>
      <div class="card">
        <div class="row">
          <div class="lbl">
            <div class="t">Current wallpaper</div>
            <div class="d file" title={s.videoPath ?? ""}>
              {fileName ?? "No video chosen — showing the animated background."}
            </div>
          </div>
        </div>
        <div class="row actions">
          <button class="btn primary" onclick={browse}>Choose video…</button>
          {#if s.videoPath}
            <button class="btn" onclick={clearVideo}>Use animated background</button>
          {/if}
        </div>
        {#if status}
          <div class="row">
            <p class="msg" class:bad={!status.ok} role="status">{status.message}</p>
          </div>
        {/if}
      </div>

      <h2 class="cap">Playback</h2>
      <div class="card">
        <div class="row stack">
          <div class="lbl">
            <div class="t">Loop transition</div>
            <div class="d">
              Seamless hands over to a pre-loaded copy with no gap. A fade blends the end into the
              start, which helps with videos that aren't made to loop.
            </div>
          </div>
          {@render segmented("loop", "Loop transition", LOOP_OPTIONS, s.loopFade, (v) => {
            s.loopFade = v;
            save(true);
          })}
        </div>
        {@render slider("speed", "Speed", 0.25, 2, 0.05, "×")}
      </div>

      <h2 class="cap">Sound</h2>
      <div class="card">
        {@render toggle("mute", "Mute", "", s.muted, (v) => {
          s.muted = v;
          save(true);
        })}
        {@render slider("volume", "Volume", 0, 100, 1, "%", 100, s.muted)}
      </div>
    {:else if tab === "picture"}
      <h2 class="cap">Adjustments</h2>
      <div class="card">
        {@render slider("brightness", "Brightness", 20, 200, 1, "%")}
        {@render slider("contrast", "Contrast", 50, 200, 1, "%")}
        {@render slider("saturation", "Saturation", 0, 200, 1, "%")}
        {@render slider("hue", "Hue", -180, 180, 1, "°")}
        {@render slider("blur", "Blur", 0, 20, 0.5, " px")}
        {@render slider("vignette", "Vignette", 0, 100, 1, "%")}
        <div class="row actions">
          <button class="btn" onclick={resetPicture}>Reset picture</button>
        </div>
      </div>
      <p class="note">
        Changes apply live. Double-click a slider to reset just that one. Adjustments are done by
        the GPU; blur is the most expensive, so keep it low on weaker graphics cards.
      </p>
    {:else if tab === "performance"}
      <h2 class="cap">Frame rate</h2>
      <div class="card">
        <div class="row stack">
          <div class="lbl">
            <div class="t">Frame limit</div>
            <div class="d">
              Limits how often frames are drawn to the desktop. Decoding still runs at the video's
              own rate, and a limit above the video's frame rate changes nothing. Unlimited shows
              the video directly, which is the cheapest path for the GPU.
            </div>
          </div>
          {@render segmented("fps", "Frame limit", FPS_OPTIONS, s.fpsLimit, (v) => {
            s.fpsLimit = v;
            save(true);
          })}
        </div>
      </div>

      <h2 class="cap">Pause to save power</h2>
      <div class="card">
        {@render toggle(
          "pause-covered",
          "When windows cover the desktop",
          "Stops decoding while other windows completely hide the wallpaper.",
          s.pauseWhenCovered,
          (v) => {
            s.pauseWhenCovered = v;
            save(true);
          },
        )}
        {@render toggle(
          "pause-battery",
          "When the PC isn't plugged in",
          "",
          s.pauseOnBattery,
          (v) => {
            s.pauseOnBattery = v;
            save(true);
          },
        )}
      </div>

      <h2 class="cap">Graphics hardware</h2>
      <div class="card">
        <div class="row">
          <div class="lbl">
            <div class="t">Graphics adapter</div>
            <div class="d">{gpu ?? "Not reported by the webview"}</div>
          </div>
        </div>
        <div class="tbl">
          <table>
            <thead>
              <tr><th>Format</th><th>Playback</th><th>Hardware decode</th></tr>
            </thead>
            <tbody>
              {#each support as c (c.name)}
                <tr>
                  <td>{c.name}</td>
                  <td class:yes={c.playback === "Yes"} class:no={c.playback === "No"}>{c.playback}</td>
                  <td class:yes={c.hardware === "Yes"} class:no={c.hardware === "No"}>{c.hardware}</td>
                </tr>
              {/each}
            </tbody>
          </table>
        </div>
      </div>
      <p class="note">
        Videos in a format marked “Yes” under Hardware decode are decoded by the GPU, which uses far
        less power than software decoding. “No” means the CPU does the work; re-encoding to H.264 or
        HEVC usually fixes that. Hardware decode is checked at 1080p.
      </p>
    {:else}
      <h2 class="cap">Startup</h2>
      <div class="card">
        {@render toggle(
          "autostart",
          "Start with Windows",
          "Launches LiveWallpaper when you sign in.",
          autostart,
          (_v, el) => void setAutostart(el),
        )}
        {#if autostartError}
          <div class="row">
            <p class="msg bad" role="status">{autostartError}</p>
          </div>
        {/if}
      </div>
    {/if}
  </div>
</div>

<style>
  :global(:root) {
    color-scheme: dark;
    --bg: #12102b;
    --surface: #1b1940;
    --surface-2: #25224f;
    --surface-3: #33306a;
    --line: #2d2a5a;
    --text: #ecebff;
    --muted: #a8a6d4;
    --accent: #8ea2ff;
    --accent-ink: #0f0d2a;
    --bad: #ff9bb3;
    --good: #7be0b0;
  }

  :global(html),
  :global(body) {
    margin: 0;
    height: 100%;
    background: var(--bg);
    color: var(--text);
    font: 14px/1.45 "Segoe UI Variable Text", "Segoe UI", system-ui, sans-serif;
  }

  :global(:focus-visible) {
    outline: 2px solid var(--accent);
    outline-offset: 2px;
  }

  .app {
    display: flex;
    flex-direction: column;
    height: 100vh;
  }

  /* ---- Header + tabs ---- */
  header {
    flex: none;
    padding: 16px 20px 0;
    border-bottom: 1px solid var(--line);
  }
  h1 {
    margin: 0 0 12px;
    font-size: 20px;
    font-weight: 650;
  }
  .tabs {
    display: flex;
    gap: 4px;
  }
  .tabs button {
    padding: 8px 12px 10px;
    border: 0;
    border-bottom: 2px solid transparent;
    background: none;
    color: var(--muted);
    font: inherit;
    cursor: pointer;
  }
  .tabs button:hover {
    color: var(--text);
  }
  .tabs button.active {
    color: var(--text);
    border-bottom-color: var(--accent);
    font-weight: 600;
  }

  /* ---- Content ---- */
  .panel {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    padding: 4px 20px 24px;
    scrollbar-color: var(--surface-3) transparent;
  }
  .cap {
    margin: 18px 4px 8px;
    font-size: 13px;
    font-weight: 600;
    color: var(--muted);
  }
  .card {
    background: var(--surface);
    border: 1px solid var(--line);
    border-radius: 10px;
    overflow: hidden;
  }
  .note {
    margin: 10px 4px 0;
    color: var(--muted);
    font-size: 12.5px;
  }

  /* ---- Rows ---- */
  .row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
    min-height: 48px;
    padding: 10px 16px;
    box-sizing: border-box;
  }
  .row + .row,
  .row + .srow,
  .srow + .srow,
  .srow + .row {
    border-top: 1px solid var(--line);
  }
  .row.stack {
    flex-direction: column;
    align-items: stretch;
    gap: 10px;
  }
  .row.actions {
    justify-content: flex-start;
    flex-wrap: wrap;
    gap: 8px;
  }
  .lbl {
    min-width: 0;
  }
  .t {
    font-weight: 500;
  }
  .d {
    color: var(--muted);
    font-size: 12.5px;
  }
  .file {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .msg {
    margin: 0;
    color: var(--muted);
  }
  .msg.bad {
    color: var(--bad);
  }

  /* ---- Buttons ---- */
  .btn {
    padding: 6px 14px;
    border: 1px solid var(--line);
    border-radius: 6px;
    background: var(--surface-2);
    color: inherit;
    font: inherit;
    cursor: pointer;
  }
  .btn:hover {
    border-color: var(--accent);
  }
  .btn.primary {
    background: var(--accent);
    border-color: var(--accent);
    color: var(--accent-ink);
    font-weight: 600;
  }

  /* ---- Switch ---- */
  .switch {
    position: relative;
    flex: none;
    width: 40px;
    height: 22px;
  }
  .switch input {
    position: absolute;
    inset: 0;
    z-index: 1;
    margin: 0;
    opacity: 0;
    cursor: pointer;
  }
  .switch .track {
    position: absolute;
    inset: 0;
    box-sizing: border-box;
    border: 1px solid var(--surface-3);
    border-radius: 11px;
    background: var(--surface-2);
    transition: background 0.15s;
  }
  .switch .track::after {
    content: "";
    position: absolute;
    top: 3px;
    left: 3px;
    width: 14px;
    height: 14px;
    border-radius: 50%;
    background: var(--muted);
    transition:
      transform 0.15s,
      background 0.15s;
  }
  .switch input:checked + .track {
    background: var(--accent);
    border-color: var(--accent);
  }
  .switch input:checked + .track::after {
    transform: translateX(18px);
    background: var(--accent-ink);
  }
  .switch input:focus-visible + .track {
    outline: 2px solid var(--accent);
    outline-offset: 2px;
  }

  /* ---- Segmented control ---- */
  .seg {
    display: flex;
    border: 1px solid var(--line);
    border-radius: 8px;
    overflow: hidden;
    background: var(--surface-2);
  }
  .seg label {
    position: relative;
    flex: 1;
  }
  .seg input {
    position: absolute;
    inset: 0;
    margin: 0;
    opacity: 0;
    cursor: pointer;
  }
  .seg span {
    display: block;
    padding: 6px 8px;
    border-right: 1px solid var(--line);
    text-align: center;
    white-space: nowrap;
  }
  .seg label:last-child span {
    border-right: 0;
  }
  .seg input:checked + span {
    background: var(--accent);
    color: var(--accent-ink);
    font-weight: 600;
  }
  .seg input:focus-visible + span {
    outline: 2px solid var(--accent);
    outline-offset: -2px;
  }

  /* ---- Sliders ---- */
  .srow {
    padding: 10px 16px 12px;
  }
  .srow .head {
    display: flex;
    justify-content: space-between;
    align-items: baseline;
  }
  .srow output {
    color: var(--muted);
    font-variant-numeric: tabular-nums;
  }
  .srow input[type="range"] {
    display: block;
    width: 100%;
    height: 20px;
    margin: 6px 0 0;
    background: transparent;
    appearance: none;
    -webkit-appearance: none;
    cursor: pointer;
  }
  .srow input[type="range"]:disabled {
    opacity: 0.45;
    cursor: default;
  }
  .srow input[type="range"]::-webkit-slider-runnable-track {
    height: 4px;
    border-radius: 2px;
    background: linear-gradient(
      to right,
      var(--accent) var(--pct, 0%),
      var(--surface-3) var(--pct, 0%)
    );
  }
  .srow input[type="range"]::-webkit-slider-thumb {
    -webkit-appearance: none;
    width: 16px;
    height: 16px;
    margin-top: -6px;
    box-sizing: border-box;
    border: 3px solid var(--surface);
    border-radius: 50%;
    background: var(--accent);
    box-shadow: 0 0 0 1px var(--accent);
  }

  /* ---- Hardware table ---- */
  .tbl {
    border-top: 1px solid var(--line);
    padding: 4px 16px 8px;
  }
  table {
    width: 100%;
    border-collapse: collapse;
    font-size: 13px;
  }
  th {
    padding: 6px 0;
    text-align: left;
    font-weight: 600;
    color: var(--muted);
  }
  td {
    padding: 4px 0;
    border-top: 1px solid var(--line);
    color: var(--muted);
  }
  td:first-child {
    color: var(--text);
  }
  td.yes {
    color: var(--good);
    font-weight: 600;
  }
  td.no {
    color: var(--muted);
    opacity: 0.7;
  }
</style>
