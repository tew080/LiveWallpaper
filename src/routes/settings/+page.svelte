<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { DEFAULT_SETTINGS, FPS_OPTIONS, type Settings } from "$lib/types";

  let s = $state<Settings>({ ...DEFAULT_SETTINGS });
  let status = $state<{ ok: boolean; message?: string } | null>(null);
  let support = $state<{ name: string; level: string }[]>([]);
  let timer: ReturnType<typeof setTimeout> | undefined;
  let autostart = $state(false);
  let autostartError = $state<string | null>(null);

  const fileName = $derived(s.videoPath?.split(/[\\/]/).pop() ?? null);

  const probes: [string, string][] = [
    ["H.264 video", 'video/mp4; codecs="avc1.640028"'],
    ["H.265 / HEVC video", 'video/mp4; codecs="hvc1.1.6.L93.B0"'],
    ["VP8 video", 'video/webm; codecs="vp8"'],
    ["VP9 video", 'video/webm; codecs="vp09.00.10.08"'],
    ["AV1 video", 'video/mp4; codecs="av01.0.08M.08"'],
    ["AAC audio", 'audio/mp4; codecs="mp4a.40.2"'],
    ["Opus audio", 'audio/webm; codecs="opus"'],
    ["Vorbis audio", 'audio/ogg; codecs="vorbis"'],
    ["MP3 audio", "audio/mpeg"],
    ["FLAC audio", "audio/flac"],
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

  async function toggleAutostart(e: Event) {
    const input = e.currentTarget as HTMLInputElement;
    try {
      autostart = await invoke<boolean>("set_autostart", { enabled: input.checked });
      autostartError = null;
    } catch (err) {
      input.checked = autostart; // revert the box to what Windows actually has
      autostartError = String(err);
    }
  }

  onMount(() => {
    const probe = document.createElement("video");
    support = probes.map(([name, type]) => {
      const r = probe.canPlayType(type);
      return { name, level: r === "probably" ? "Yes" : r === "maybe" ? "Maybe" : "No" };
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

<main>
  <h1>LiveWallpaper</h1>

  <section aria-labelledby="h-video">
    <h2 id="h-video">Video</h2>
    <p class="file" title={s.videoPath ?? ""}>
      {fileName ?? "No video chosen — showing the animated background."}
    </p>
    <div class="row">
      <button class="primary" onclick={browse}>Choose video…</button>
      {#if s.videoPath}
        <button onclick={clearVideo}>Use animated background</button>
      {/if}
    </div>
    {#if status}
      <p class="msg" class:bad={!status.ok} role="status">{status.message}</p>
    {/if}
  </section>

  <section aria-labelledby="h-fps">
    <h2 id="h-fps">Frame rate</h2>
    <div class="seg" role="radiogroup" aria-labelledby="h-fps">
      {#each FPS_OPTIONS as o (o.value)}
        <label>
          <input
            type="radio"
            name="fps"
            value={o.value}
            checked={s.fpsLimit === o.value}
            onchange={() => {
              s.fpsLimit = o.value;
              save(true);
            }}
          />
          <span>{o.label}</span>
        </label>
      {/each}
    </div>
    <p class="hint">
      Limits how often frames are drawn to the desktop. Decoding still runs at the video's own
      rate, and a limit above the video's frame rate changes nothing. Unlimited shows the video
      directly.
    </p>
  </section>

  <section aria-labelledby="h-sound">
    <h2 id="h-sound">Sound</h2>
    <label class="slider">
      <span>Volume</span>
      <input
        type="range"
        min="0"
        max="100"
        value={Math.round(s.volume * 100)}
        disabled={s.muted}
        oninput={(e) => {
          s.volume = Number(e.currentTarget.value) / 100;
          save();
        }}
      />
      <output>{Math.round(s.volume * 100)}%</output>
    </label>
    <label class="check">
      <input type="checkbox" bind:checked={s.muted} onchange={() => save(true)} />
      Mute
    </label>
  </section>

  <section aria-labelledby="h-power">
    <h2 id="h-power">Pause to save power</h2>
    <label class="check">
      <input type="checkbox" bind:checked={s.pauseWhenCovered} onchange={() => save(true)} />
      When other windows completely cover the desktop
    </label>
    <label class="check">
      <input type="checkbox" bind:checked={s.pauseOnBattery} onchange={() => save(true)} />
      When the PC isn't plugged in
    </label>
  </section>

  <section aria-labelledby="h-startup">
    <h2 id="h-startup">Startup</h2>
    <label class="check">
      <input type="checkbox" checked={autostart} onchange={toggleAutostart} />
      Start LiveWallpaper when I sign in to Windows
    </label>
    {#if autostartError}
      <p class="msg bad" role="status">{autostartError}</p>
    {/if}
  </section>

  <section aria-labelledby="h-codecs">
    <h2 id="h-codecs">Playback support on this PC</h2>
    <ul class="codecs">
      {#each support as c (c.name)}
        <li><span>{c.name}</span><b class:no={c.level === "No"}>{c.level}</b></li>
      {/each}
    </ul>
    <p class="hint">
      Playback uses the Windows WebView2 engine, so only these codecs can play. Formats such as
      AVI or WMV usually can't.
    </p>
  </section>
</main>

<style>
  :global(html),
  :global(body) {
    margin: 0;
    background: #14122e;
    color: #e9e7ff;
    font: 14px/1.45 "Segoe UI Variable Text", "Segoe UI", system-ui, sans-serif;
  }

  main {
    --line: #2d2a56;
    --soft: #a9a6d4;
    --accent: #8ea2ff;
    padding: 20px 24px 28px;
  }

  h1 {
    margin: 0 0 14px;
    font-size: 22px;
    font-weight: 650;
  }

  section {
    padding: 14px 0;
    border-top: 1px solid var(--line);
  }

  h2 {
    margin: 0 0 8px;
    font-size: 14px;
    font-weight: 650;
  }

  .file {
    margin: 0 0 10px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    color: var(--soft);
  }

  .row {
    display: flex;
    gap: 8px;
    flex-wrap: wrap;
  }

  button {
    padding: 6px 12px;
    border: 1px solid var(--line);
    border-radius: 6px;
    background: #1e1b42;
    color: inherit;
    font: inherit;
    cursor: pointer;
  }
  button:hover {
    border-color: var(--accent);
  }
  button.primary {
    background: var(--accent);
    border-color: var(--accent);
    color: #0f0d2a;
    font-weight: 600;
  }

  :global(:focus-visible) {
    outline: 2px solid var(--accent);
    outline-offset: 2px;
  }

  .msg {
    margin: 10px 0 0;
    color: var(--soft);
  }
  .msg.bad {
    color: #ffb4c4;
  }

  .hint {
    margin: 8px 0 0;
    color: var(--soft);
    font-size: 12.5px;
  }

  .seg {
    display: inline-flex;
    border: 1px solid var(--line);
    border-radius: 6px;
    overflow: hidden;
  }
  .seg label {
    position: relative;
  }
  .seg input {
    position: absolute;
    opacity: 0;
    inset: 0;
    margin: 0;
    cursor: pointer;
  }
  .seg span {
    display: block;
    padding: 6px 14px;
    border-right: 1px solid var(--line);
  }
  .seg label:last-child span {
    border-right: 0;
  }
  .seg input:checked + span {
    background: var(--accent);
    color: #0f0d2a;
    font-weight: 600;
  }
  .seg input:focus-visible + span {
    outline: 2px solid var(--accent);
    outline-offset: -2px;
  }

  .slider {
    display: grid;
    grid-template-columns: 64px 1fr 44px;
    align-items: center;
    gap: 8px;
    margin-bottom: 8px;
  }
  .slider output {
    text-align: right;
    color: var(--soft);
  }
  input[type="range"] {
    accent-color: var(--accent);
  }

  .check {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 3px 0;
  }
  input[type="checkbox"] {
    accent-color: var(--accent);
  }

  .codecs {
    margin: 0;
    padding: 0;
    list-style: none;
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 2px 24px;
  }
  .codecs li {
    display: flex;
    justify-content: space-between;
  }
  .codecs b {
    font-weight: 600;
    color: var(--accent);
  }
  .codecs b.no {
    color: #8a87b5;
    font-weight: 400;
  }
</style>
