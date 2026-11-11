<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { emit, listen, type UnlistenFn } from "@tauri-apps/api/event";
  import {
    BATTERY_FPS_OPTIONS,
    DEFAULT_SETTINGS,
    FIT_OPTIONS,
    FPS_OPTIONS,
    LANGUAGE_OPTIONS,
    LOOP_OPTIONS,
    POWER_MODES,
    PRESETS,
    PROFILE_OPTIONS,
    type Settings,
  } from "$lib/types";
  import type { RendererInfo } from "$lib/gl";
  import { setLanguage, t } from "$lib/i18n.svelte";

  const TABS = ["wallpaper", "picture", "effects", "performance", "general"] as const;
  type Tab = (typeof TABS)[number];

  const PICTURE_KEYS = [
    "brightness",
    "contrast",
    "saturation",
    "hue",
    "grayscale",
    "sepia",
    "blur",
    "temperature",
  ] as const;
  const EFFECT_KEYS = ["sharpen", "grain", "scanlines", "vignette", "aberration", "pixelate", "tintStrength"] as const;
  type NumKey =
    | (typeof PICTURE_KEYS)[number]
    | (typeof EFFECT_KEYS)[number]
    | "volume"
    | "speed"
    | "renderScale";

  type Gpu = { name: string; kind: string };

  let tab = $state<Tab>("wallpaper");
  let s = $state<Settings>({ ...DEFAULT_SETTINGS });
  let status = $state<{ ok: boolean; message?: string } | null>(null);
  let support = $state<{ name: string; playback: string; hardware: string }[]>([]);
  let gpu = $state<string | null>(null);
  let gpus = $state<Gpu[] | null>(null);
  let gpuError = $state<string | null>(null);
  let renderer = $state<RendererInfo | null>(null);
  let timer: ReturnType<typeof setTimeout> | undefined;
  let autostart = $state(false);
  let autostartError = $state<string | null>(null);
  // Hardware options as they were when the app started; they only apply after a restart.
  let applied = $state<{ gpu: boolean; decode: boolean; profile: string; choice: string } | null>(null);
  const restartNeeded = $derived(
    applied !== null &&
      (applied.gpu !== s.gpuAcceleration ||
        applied.decode !== s.hwVideoDecode ||
        applied.profile !== s.gpuProfile ||
        applied.choice !== s.gpuChoice),
  );

  $effect(() => setLanguage(s.language));

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

  function resetKeys(keys: readonly NumKey[]) {
    for (const k of keys) s[k] = DEFAULT_SETTINGS[k];
  }

  function resetPicture() {
    resetKeys(PICTURE_KEYS);
    save(true);
  }

  function resetEffects() {
    resetKeys(EFFECT_KEYS);
    s.tintColor = DEFAULT_SETTINGS.tintColor;
    save(true);
  }

  const matches = (values: Partial<Settings>) =>
    (Object.keys(values) as (keyof Settings)[]).every((k) => s[k] === values[k]);

  function applyPreset(values: Partial<Settings>) {
    resetKeys(PICTURE_KEYS);
    resetKeys(EFFECT_KEYS);
    s.tintColor = DEFAULT_SETTINGS.tintColor;
    Object.assign(s, values);
    save(true);
  }

  function applyPower(values: Partial<Settings>) {
    Object.assign(s, values);
    save(true);
  }

  const choiceFor = (g: Gpu) => (g.kind === "discrete" ? "performance" : "power");
  // Two adapters of the same kind can't be told apart by Windows' preference; the first one is shown as chosen.
  const isChosen = (g: Gpu, i: number) =>
    s.gpuChoice === choiceFor(g) && (gpus ?? []).findIndex((x) => x.kind === g.kind) === i;

  function chooseGpu(choice: Settings["gpuChoice"]) {
    s.gpuChoice = choice;
    save(true);
  }

  const run = (command: string) =>
    invoke(command).catch((e) => (status = { ok: false, message: String(e) }));

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
    const i = TABS.indexOf(tab);
    let n = i;
    if (e.key === "ArrowRight") n = (i + 1) % TABS.length;
    else if (e.key === "ArrowLeft") n = (i - 1 + TABS.length) % TABS.length;
    else return;
    e.preventDefault();
    tab = TABS[n];
    queueMicrotask(() => document.getElementById(`tab-${tab}`)?.focus());
  }

  const fmt = (v: number) => String(Math.round(v * 100) / 100);
  const answer = (v: string) => (v === "Yes" ? t("yes") : v === "No" ? t("no") : v === "Maybe" ? t("maybe") : v);

  const rendererText = $derived.by(() => {
    if (!renderer) return t("rend.waiting");
    if (renderer.reason) return `${t("rend.css")} — ${renderer.reason}`;
    return t(renderer.mode === "gl" ? "rend.gl" : renderer.mode === "canvas" ? "rend.canvas" : "rend.direct");
  });
  const noScaleHint = $derived(
    !!renderer &&
      renderer.mode === "gl" &&
      s.smoothScaling &&
      renderer.vw > 0 &&
      renderer.vw <= renderer.cw * 1.01 &&
      renderer.vh <= renderer.ch * 1.01,
  );

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
    const offs: UnlistenFn[] = [];

    invoke<Settings>("get_settings").then((v) => {
      s = v;
      applied = { gpu: v.gpuAcceleration, decode: v.hwVideoDecode, profile: v.gpuProfile, choice: v.gpuChoice };
    });
    invoke<boolean>("get_autostart").then((v) => (autostart = v));
    invoke<Gpu[]>("list_gpus")
      .then((v) => (gpus = v))
      .catch((e) => (gpuError = String(e)));

    Promise.all([
      listen<{ ok: boolean; message?: string }>("wallpaper:status", (e) => {
        status = e.payload.ok && !e.payload.message ? null : e.payload;
      }),
      listen<RendererInfo>("wallpaper:renderer", (e) => (renderer = e.payload)),
    ]).then((fns) => {
      if (disposed) fns.forEach((off) => off());
      else offs.push(...fns);
      emit("renderer:request").catch(() => {});
    });

    return () => {
      disposed = true;
      offs.forEach((off) => off());
      clearTimeout(timer);
    };
  });
</script>

<svelte:head>
  <title>{t("title")}</title>
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
  options: { value: number | string; key: string }[],
  current: number | string,
  pick: (value: any) => void,
)}
  <div class="seg" role="radiogroup" aria-label={label}>
    {#each options as o (o.value)}
      <label>
        <input type="radio" {name} value={o.value} checked={current === o.value} onchange={() => pick(o.value)} />
        <span>{t(o.key)}</span>
      </label>
    {/each}
  </div>
{/snippet}

<div class="app">
  <header>
    <h1>LiveWallpaper</h1>
    <div class="tabs" role="tablist" aria-label={t("aria.tabs")}>
      {#each TABS as id (id)}
        <button
          role="tab"
          id="tab-{id}"
          aria-selected={tab === id}
          aria-controls="panel"
          tabindex={tab === id ? 0 : -1}
          class:active={tab === id}
          onclick={() => (tab = id)}
          onkeydown={onTabKey}
        >
          {t(`tab.${id}`)}
        </button>
      {/each}
    </div>
  </header>

  <div class="panel" id="panel" role="tabpanel" aria-labelledby="tab-{tab}">
    {#if tab === "wallpaper"}
      <h2 class="cap">{t("cap.video")}</h2>
      <div class="card">
        <div class="row">
          <div class="lbl">
            <div class="t">{t("wp.current")}</div>
            <div class="d file" title={s.videoPath ?? ""}>
              {fileName ?? t("wp.none")}
            </div>
          </div>
        </div>
        <div class="row actions">
          <button class="btn primary" onclick={browse}>{t("wp.choose")}</button>
          {#if s.videoPath}
            <button class="btn" onclick={clearVideo}>{t("wp.useAnimated")}</button>
          {/if}
        </div>
        {#if status}
          <div class="row">
            <p class="msg" class:bad={!status.ok} role="status">{status.message}</p>
          </div>
        {/if}
      </div>

      <h2 class="cap">{t("cap.display")}</h2>
      <div class="card">
        <div class="row stack">
          <div class="lbl">
            <div class="t">{t("fit.title")}</div>
            <div class="d">{t("fit.desc")}</div>
          </div>
          {@render segmented("fit", t("fit.title"), FIT_OPTIONS, s.fit, (v) => {
            s.fit = v;
            save(true);
          })}
        </div>
      </div>

      <h2 class="cap">{t("cap.playback")}</h2>
      <div class="card">
        <div class="row stack">
          <div class="lbl">
            <div class="t">{t("loop.title")}</div>
            <div class="d">{t("loop.desc")}</div>
          </div>
          {@render segmented("loop", t("loop.title"), LOOP_OPTIONS, s.loopFade, (v) => {
            s.loopFade = v;
            save(true);
          })}
        </div>
        {@render slider("speed", t("speed"), 0.25, 2, 0.05, "×")}
      </div>

      <h2 class="cap">{t("cap.sound")}</h2>
      <div class="card">
        {@render toggle("mute", t("mute"), "", s.muted, (v) => {
          s.muted = v;
          save(true);
        })}
        {@render slider("volume", t("volume"), 0, 100, 1, "%", 100, s.muted)}
      </div>
    {:else if tab === "picture"}
      <h2 class="cap">{t("cap.adjustments")}</h2>
      <div class="card">
        {@render slider("brightness", t("brightness"), 20, 200, 1, "%")}
        {@render slider("contrast", t("contrast"), 50, 200, 1, "%")}
        {@render slider("saturation", t("saturation"), 0, 200, 1, "%")}
        {@render slider("hue", t("hue"), -180, 180, 1, "°")}
        {@render slider("temperature", t("temperature"), -100, 100, 1, "")}
        {@render slider("grayscale", t("grayscale"), 0, 100, 1, "%")}
        {@render slider("sepia", t("sepia"), 0, 100, 1, "%")}
        {@render slider("blur", t("blur"), 0, 20, 0.5, " px")}
        <div class="row actions">
          <button class="btn" onclick={resetPicture}>{t("pic.reset")}</button>
        </div>
      </div>
      <p class="note">{t("pic.note")}</p>
    {:else if tab === "effects"}
      <h2 class="cap">{t("cap.renderer")}</h2>
      <div class="card">
        <div class="row">
          <div class="lbl">
            <div class="t">{rendererText}</div>
            {#if renderer && renderer.vw > 0}
              <div class="d">{t("rend.size", { vw: renderer.vw, vh: renderer.vh, cw: renderer.cw, ch: renderer.ch })}</div>
            {/if}
            {#if noScaleHint}
              <div class="d">{t("rend.noScale")}</div>
            {/if}
          </div>
        </div>
      </div>

      <h2 class="cap">{t("cap.presets")}</h2>
      <div class="card">
        <div class="chips">
          <button class="chip" onclick={() => applyPreset({})}>{t("preset.none")}</button>
          {#each PRESETS as p (p.id)}
            <button class="chip" class:on={matches(p.values)} onclick={() => applyPreset(p.values)}>
              {t(`preset.${p.id}`)}
            </button>
          {/each}
        </div>
      </div>
      <p class="note">{t("presets.note")}</p>

      <h2 class="cap">{t("cap.quality")}</h2>
      <div class="card">
        {@render toggle("smooth", t("smooth.title"), t("smooth.desc"), s.smoothScaling, (v) => {
          s.smoothScaling = v;
          save(true);
        })}
        {@render toggle("aa", t("aa.title"), t("aa.desc"), s.antiAliasing, (v) => {
          s.antiAliasing = v;
          save(true);
        })}
        {@render slider("sharpen", t("sharpen"), 0, 100, 1, "%")}
      </div>
      <p class="note">{t("quality.note")}</p>

      <h2 class="cap">{t("cap.effects")}</h2>
      <div class="card">
        {@render slider("grain", t("grain"), 0, 100, 1, "%")}
        {@render slider("scanlines", t("scanlines"), 0, 100, 1, "%")}
        {@render slider("vignette", t("vignette"), 0, 100, 1, "%")}
        {@render slider("aberration", t("aberration"), 0, 100, 1, "%")}
        {@render slider("pixelate", t("pixelate"), 0, 100, 1, "%")}
        <div class="row">
          <div class="lbl">
            <label class="t" for="tint-color">{t("tint.color")}</label>
            <div class="d">{t("tint.desc")}</div>
          </div>
          <input
            id="tint-color"
            class="color"
            type="color"
            value={s.tintColor}
            oninput={(e) => {
              s.tintColor = e.currentTarget.value;
              save();
            }}
          />
        </div>
        {@render slider("tintStrength", t("tint.strength"), 0, 100, 1, "%")}
        <div class="row actions">
          <button class="btn" onclick={resetEffects}>{t("fx.reset")}</button>
        </div>
      </div>
      <p class="note">{t("fx.note")}</p>
    {:else if tab === "performance"}
      {#if restartNeeded}
        <div class="card">
          <div class="row restart">
            <div class="lbl">
              <div class="t">{t("restart.t")}</div>
              <div class="d">{t("restart.d")}</div>
            </div>
            <button class="btn primary" onclick={() => run("restart_app")}>{t("restart.btn")}</button>
          </div>
        </div>
      {/if}

      <h2 class="cap">{t("cap.powerMode")}</h2>
      <div class="card">
        <div class="chips">
          {#each POWER_MODES as m (m.id)}
            <button class="chip" class:on={matches(m.values)} onclick={() => applyPower(m.values)}>
              {t(`power.${m.id}`)}
            </button>
          {/each}
        </div>
      </div>
      <p class="note">{t("power.desc")}</p>

      <h2 class="cap">{t("cap.frameRate")}</h2>
      <div class="card">
        <div class="row stack">
          <div class="lbl">
            <div class="t">{t("fps.title")}</div>
            <div class="d">{t("fps.desc")}</div>
          </div>
          {@render segmented("fps", t("fps.title"), FPS_OPTIONS, s.fpsLimit, (v) => {
            s.fpsLimit = v;
            save(true);
          })}
        </div>
        <div class="row stack">
          <div class="lbl">
            <div class="t">{t("bfps.title")}</div>
            <div class="d">{t("bfps.desc")}</div>
          </div>
          {@render segmented("bfps", t("bfps.title"), BATTERY_FPS_OPTIONS, s.batteryFps, (v) => {
            s.batteryFps = v;
            save(true);
          })}
        </div>
        {@render slider("renderScale", t("scale.title"), 50, 100, 5, "%")}
      </div>
      <p class="note">{t("scale.desc")}</p>

      <h2 class="cap">{t("cap.pause")}</h2>
      <div class="card">
        {@render toggle("pause-covered", t("pause.covered.t"), t("pause.covered.d"), s.pauseWhenCovered, (v) => {
          s.pauseWhenCovered = v;
          save(true);
        })}
        {@render toggle("pause-battery", t("pause.battery.t"), "", s.pauseOnBattery, (v) => {
          s.pauseOnBattery = v;
          save(true);
        })}
      </div>

      <h2 class="cap">{t("cap.gpu")}</h2>
      <div class="card">
        <div class="row">
          <div class="lbl">
            <div class="d">{t("gpu.desc")}</div>
          </div>
        </div>
        <div class="gpus" role="radiogroup" aria-label={t("cap.gpu")}>
          <label class="gpu">
            <input type="radio" name="gpu" checked={s.gpuChoice === "auto"} onchange={() => chooseGpu("auto")} />
            <span class="gtxt">
              <span class="name">{t("gpu.auto")}</span>
              <span class="sub">{t("gpu.autoD")}</span>
            </span>
          </label>
          {#each gpus ?? [] as g, i (g.name + i)}
            <label class="gpu">
              <input
                type="radio"
                name="gpu"
                checked={isChosen(g, i)}
                onchange={() => chooseGpu(choiceFor(g))}
              />
              <span class="gtxt">
                <span class="name">{g.name}</span>
                <span class="sub">{t(g.kind === "discrete" ? "gpu.discreteD" : "gpu.integratedD")}</span>
              </span>
              <span class="badge">{t(g.kind === "discrete" ? "gpu.discrete" : "gpu.integrated")}</span>
            </label>
          {/each}
        </div>
        {#if gpuError}
          <p class="msg bad pad">{t("gpu.failed", { error: gpuError })}</p>
        {:else if gpus === null}
          <p class="d pad">{t("gpu.detecting")}</p>
        {:else if gpus.length <= 1}
          <p class="d pad">{t("gpu.single")}</p>
        {/if}
        <div class="row">
          <div class="lbl">
            <div class="d">{t("gpu.inUse", { name: gpu ?? t("gpu.unknown") })}</div>
          </div>
          <button class="btn" onclick={() => run("open_graphics_settings")}>{t("gpu.open")}</button>
        </div>
      </div>

      <h2 class="cap">{t("cap.hardware")}</h2>
      <div class="card">
        {@render toggle("gpu-accel", t("gpuaccel.t"), t("gpuaccel.d"), s.gpuAcceleration, (v) => {
          s.gpuAcceleration = v;
          save(true);
        })}
        {@render toggle("hw-decode", t("hwdec.t"), t("hwdec.d"), s.hwVideoDecode, (v) => {
          s.hwVideoDecode = v;
          save(true);
        })}
        <div class="row stack">
          <div class="lbl">
            <div class="t">{t("profile.t")}</div>
            <div class="d">{t("profile.d")}</div>
          </div>
          {@render segmented("profile", t("profile.t"), PROFILE_OPTIONS, s.gpuProfile, (v) => {
            s.gpuProfile = v;
            save(true);
          })}
        </div>
      </div>

      <h2 class="cap">{t("cap.formats")}</h2>
      <div class="card">
        <div class="tbl">
          <table>
            <thead>
              <tr><th>{t("fmt.format")}</th><th>{t("fmt.playback")}</th><th>{t("fmt.hw")}</th></tr>
            </thead>
            <tbody>
              {#each support as c (c.name)}
                <tr>
                  <td>{c.name}</td>
                  <td class:yes={c.playback === "Yes"} class:no={c.playback === "No"}>{answer(c.playback)}</td>
                  <td class:yes={c.hardware === "Yes"} class:no={c.hardware === "No"}>{answer(c.hardware)}</td>
                </tr>
              {/each}
            </tbody>
          </table>
        </div>
      </div>
      <p class="note">{t("fmt.note")}</p>
    {:else}
      <h2 class="cap">{t("cap.language")}</h2>
      <div class="card">
        <div class="row stack">
          <div class="lbl">
            <div class="t">{t("lang.t")}</div>
            <div class="d">{t("lang.d")}</div>
          </div>
          {@render segmented("lang", t("lang.t"), LANGUAGE_OPTIONS, s.language, (v) => {
            s.language = v;
            save(true);
          })}
        </div>
      </div>

      <h2 class="cap">{t("cap.startup")}</h2>
      <div class="card">
        {@render toggle("autostart", t("autostart.t"), t("autostart.d"), autostart, (_v, el) => void setAutostart(el))}
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
    padding: 8px 10px 10px;
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

  .color {
    flex: none;
    width: 44px;
    height: 28px;
    padding: 0;
    border: 1px solid var(--line);
    border-radius: 6px;
    background: var(--surface-2);
    cursor: pointer;
  }
  .row.restart {
    background: var(--surface-2);
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

  /* ---- Presets, GPU list ---- */
  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
    padding: 12px 14px;
  }
  .chip {
    padding: 6px 14px;
    border: 1px solid var(--line);
    border-radius: 999px;
    background: var(--surface-2);
    color: var(--text);
    font: inherit;
    cursor: pointer;
  }
  .chip:hover {
    background: var(--surface-3);
  }
  .chip.on {
    background: var(--accent);
    border-color: var(--accent);
    color: var(--accent-ink);
  }
  .gpus {
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 4px 14px 12px;
  }
  .gpu {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 10px 12px;
    border: 1px solid var(--line);
    border-radius: 10px;
    background: var(--surface-2);
    cursor: pointer;
  }
  .gpu:has(input:checked) {
    border-color: var(--accent);
  }
  .gpu input {
    accent-color: var(--accent);
    margin: 0;
  }
  .gtxt {
    display: flex;
    flex: 1;
    flex-direction: column;
    min-width: 0;
  }
  .gtxt .name {
    font-weight: 600;
    overflow-wrap: anywhere;
  }
  .gtxt .sub {
    color: var(--muted);
    font-size: 12.5px;
  }
  .badge {
    flex: none;
    padding: 2px 8px;
    border-radius: 999px;
    background: var(--surface-3);
    color: var(--muted);
    font-size: 12px;
  }
  .pad {
    margin: 0;
    padding: 0 14px 12px;
  }
</style>
