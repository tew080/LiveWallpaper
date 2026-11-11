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
    if (settings.brightness !== 100) parts.push(`brightness(${settings.brightness}%)`);
    if (settings.contrast !== 100) parts.push(`contrast(${settings.contrast}%)`);
    if (settings.saturation !== 100) parts.push(`saturate(${settings.saturation}%)`);
    if (settings.hue !== 0) parts.push(`hue-rotate(${settings.hue}deg)`);
    if (settings.blur > 0) parts.push(`blur(${settings.blur}px)`);
    return parts.length ? parts.join(" ") : "none";
  });

  function onstatus(ok: boolean, message?: string) {
    void emit("wallpaper:status", { ok, message });
  }

  onMount(() => {
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

<div class="stage" class:blurred={settings.blur > 0} style:filter>
  {#if src}
    <VideoWallpaper
      {src}
      fps={settings.fpsLimit}
      volume={settings.volume}
      muted={settings.muted}
      speed={settings.speed}
      loopFade={settings.loopFade}
      {paused}
      {onstatus}
    />
  {:else}
    <FlowField fps={settings.fpsLimit} {paused} />
  {/if}
</div>

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

  .stage {
    position: fixed;
    inset: 0;
    overflow: hidden;
  }
  /* Blur fades out toward the screen edges; a slight zoom hides that. */
  .stage.blurred {
    transform: scale(1.06);
  }

  .vignette {
    position: fixed;
    inset: 0;
    pointer-events: none;
    background: radial-gradient(ellipse at center, transparent 45%, rgba(0, 0, 0, 0.9) 100%);
  }
</style>
