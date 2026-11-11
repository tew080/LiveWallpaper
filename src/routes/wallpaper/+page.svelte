<script lang="ts">
  import { onMount } from "svelte";
  import { convertFileSrc, invoke } from "@tauri-apps/api/core";
  import { emit, listen, type UnlistenFn } from "@tauri-apps/api/event";
  import FlowField from "$lib/FlowField.svelte";
  import VideoWallpaper from "$lib/VideoWallpaper.svelte";
  import { DEFAULT_SETTINGS, type RuntimeState, type Settings } from "$lib/types";

  let settings = $state<Settings>({ ...DEFAULT_SETTINGS });
  let runtime = $state<RuntimeState>({ covered: false, onBattery: false });
  let userPaused = $state(false); // tray "Pause / Resume"

  const paused = $derived(
    userPaused ||
      (settings.pauseWhenCovered && runtime.covered) ||
      (settings.pauseOnBattery && runtime.onBattery),
  );
  const src = $derived(settings.videoPath ? convertFileSrc(settings.videoPath) : null);

  // Picture adjustments run on the GPU as one CSS filter. Defaults are left
  // out so an unadjusted wallpaper costs nothing extra.
  const filter = $derived.by(() => {
    const parts: string[] = [];
    if (settings.sharpen > 0) parts.push("url(#lw-sharpen)");
    if (settings.brightness !== 100) parts.push(`brightness(${settings.brightness}%)`);
    if (settings.contrast !== 100) parts.push(`contrast(${settings.contrast}%)`);
    if (settings.saturation !== 100) parts.push(`saturate(${settings.saturation}%)`);
    if (settings.hue !== 0) parts.push(`hue-rotate(${settings.hue}deg)`);
    if (settings.grayscale > 0) parts.push(`grayscale(${settings.grayscale}%)`);
    if (settings.sepia > 0) parts.push(`sepia(${settings.sepia}%)`);
    if (settings.blur > 0) parts.push(`blur(${settings.blur}px)`);
    return parts.length ? parts.join(" ") : "none";
  });

  // Unsharp-style kernel: centre 1 + 4k, neighbours -k (sums to 1, so flat areas keep their level).
  const kernel = $derived.by(() => {
    const k = (settings.sharpen / 100) * 0.8;
    return `0 ${-k} 0 ${-k} ${1 + 4 * k} ${-k} 0 ${-k} 0`;
  });

  // Small noise tile for the film-grain overlay, drawn once.
  let noise = $state("");
  function makeNoise(): string {
    const size = 160;
    const c = document.createElement("canvas");
    c.width = c.height = size;
    const g = c.getContext("2d")!;
    const img = g.createImageData(size, size);
    for (let i = 0; i < img.data.length; i += 4) {
      const v = Math.random() * 255;
      img.data[i] = img.data[i + 1] = img.data[i + 2] = v;
      img.data[i + 3] = 255;
    }
    g.putImageData(img, 0, 0);
    return c.toDataURL();
  }

  function onstatus(ok: boolean, message?: string) {
    void emit("wallpaper:status", { ok, message });
  }

  onMount(() => {
    noise = makeNoise();
    if (!("__TAURI_INTERNALS__" in window)) return; // plain browser (npm run dev)

    let disposed = false;
    const offs: UnlistenFn[] = [];

    (async () => {
      settings = await invoke<Settings>("get_settings");
      runtime = await invoke<RuntimeState>("get_runtime");

      const subs = await Promise.all([
        listen<Settings>("settings:changed", (e) => (settings = e.payload)),
        listen<RuntimeState>("runtime:changed", (e) => (runtime = e.payload)),
        listen("wallpaper:toggle", () => (userPaused = !userPaused)),
      ]);
      if (disposed) subs.forEach((off) => off());
      else offs.push(...subs);
    })();

    return () => {
      disposed = true;
      offs.forEach((off) => off());
    };
  });
</script>

<svelte:head>
  <title>LiveWallpaper</title>
</svelte:head>

<!-- Sharpening filter, referenced from the CSS filter on .stage. -->
<svg class="defs" width="0" height="0" aria-hidden="true">
  <filter id="lw-sharpen" color-interpolation-filters="sRGB">
    <feConvolveMatrix order="3" kernelMatrix={kernel} preserveAlpha="true" />
  </filter>
</svg>

<div class="stage" class:blurred={settings.blur > 0} style:filter>
  {#if src}
    <VideoWallpaper
      {src}
      fps={settings.fpsLimit}
      volume={settings.volume}
      muted={settings.muted}
      speed={settings.speed}
      loopFade={settings.loopFade}
      smooth={settings.smoothScaling}
      fit={settings.fit}
      {paused}
      {onstatus}
    />
  {:else}
    <FlowField fps={settings.fpsLimit} {paused} />
  {/if}
</div>

{#if settings.tintStrength > 0}
  <div class="tint" style:background={settings.tintColor} style:opacity={settings.tintStrength / 100}></div>
{/if}

{#if settings.grain > 0}
  <div
    class="grain"
    class:still={paused}
    style:background-image="url({noise})"
    style:opacity={(settings.grain / 100) * 0.4}
  ></div>
{/if}

{#if settings.scanlines > 0}
  <div class="scan" style:opacity={settings.scanlines / 100}></div>
{/if}

{#if settings.vignette > 0}
  <div class="vignette" style:opacity={settings.vignette / 100}></div>
{/if}

<style>
  :global(html),
  :global(body) {
    margin: 0;
    width: 100%;
    height: 100%;
    overflow: hidden;
    background: #12102b;
    cursor: none;
    user-select: none;
  }

  .defs {
    position: fixed;
    pointer-events: none;
  }

  .stage {
    position: fixed;
    inset: 0;
    overflow: hidden;
    background: #000; /* bars when the video is set to "Fit" */
  }
  /* Blur fades out toward the screen edges; a slight zoom hides that. */
  .stage.blurred {
    transform: scale(1.06);
  }

  .tint,
  .grain,
  .scan {
    position: fixed;
    pointer-events: none;
  }
  .tint {
    inset: 0;
    mix-blend-mode: color;
  }
  /* Oversized so the shaking tile never shows an edge. */
  .grain {
    inset: -100px;
    background-size: 160px 160px;
    mix-blend-mode: overlay;
    animation: grain 0.6s steps(1, end) infinite;
  }
  .grain.still {
    animation-play-state: paused;
  }
  @keyframes grain {
    0% {
      transform: translate(0, 0);
    }
    20% {
      transform: translate(-37px, 23px);
    }
    40% {
      transform: translate(29px, -41px);
    }
    60% {
      transform: translate(-17px, -31px);
    }
    80% {
      transform: translate(43px, 17px);
    }
  }
  .scan {
    inset: 0;
    background: repeating-linear-gradient(to bottom, rgba(0, 0, 0, 0.55) 0 1px, transparent 1px 3px);
  }

  .vignette {
    position: fixed;
    inset: 0;
    pointer-events: none;
    background: radial-gradient(ellipse at center, transparent 45%, rgba(0, 0, 0, 0.9) 100%);
  }
</style>
