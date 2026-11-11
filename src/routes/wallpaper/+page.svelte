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

{#if src}
  <VideoWallpaper
    {src}
    fps={settings.fpsLimit}
    volume={settings.volume}
    muted={settings.muted}
    {paused}
    {onstatus}
  />
{:else}
  <FlowField fps={settings.fpsLimit} {paused} />
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
</style>
