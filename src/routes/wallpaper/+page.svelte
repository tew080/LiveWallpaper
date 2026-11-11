<script lang="ts">
  import { onMount } from "svelte";
  import FlowField from "$lib/FlowField.svelte";

  let paused = $state(false);

  // Tray menu "Pause / Resume" emits this event from the Rust side.
  onMount(() => {
    if (!("__TAURI_INTERNALS__" in window)) return; // plain browser (npm run dev)

    let unlisten: (() => void) | undefined;
    let disposed = false;

    import("@tauri-apps/api/event").then(async ({ listen }) => {
      const off = await listen("wallpaper:toggle", () => {
        paused = !paused;
      });
      if (disposed) off();
      else unlisten = off;
    });

    return () => {
      disposed = true;
      unlisten?.();
    };
  });
</script>

<svelte:head>
  <title>LiveWallpaper</title>
</svelte:head>

<FlowField {paused} />

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
