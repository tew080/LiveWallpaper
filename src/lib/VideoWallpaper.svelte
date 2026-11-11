<script lang="ts">
  interface Props {
    src: string;
    /** 0 = unlimited: the <video> element is shown directly. Otherwise frames are copied to a canvas at this rate. */
    fps?: number;
    volume?: number;
    muted?: boolean;
    paused?: boolean;
    onstatus?: (ok: boolean, message?: string) => void;
  }

  let { src, fps = 0, volume = 0.5, muted = true, paused = false, onstatus }: Props = $props();

  let video = $state<HTMLVideoElement>();
  let canvas = $state<HTMLCanvasElement>();
  const direct = $derived(fps === 0);

  async function play() {
    if (!video) return;
    try {
      await video.play();
      onstatus?.(true);
    } catch (e) {
      const name = e instanceof DOMException ? e.name : "";
      if (name === "AbortError") return; // a new source replaced this one
      if (name === "NotAllowedError" && !video.muted) {
        // Sound autoplay was refused: keep the picture running, silently.
        video.muted = true;
        try {
          await video.play();
          onstatus?.(true, "The webview blocked sound autoplay, so the video is playing muted.");
          return;
        } catch {
          /* fall through */
        }
      }
      onstatus?.(false, name === "NotSupportedError" ? unsupported() : String(e));
    }
  }

  function unsupported() {
    return "The webview can't decode this file (unsupported container or codec). H.264/AAC in MP4 and VP9/Opus in WebM are the safest choices.";
  }

  function onError() {
    const err = video?.error;
    onstatus?.(false, err?.code === 4 ? unsupported() : `Playback error${err?.message ? `: ${err.message}` : ""}`);
  }

  // Sound
  $effect(() => {
    if (!video) return;
    video.volume = Math.min(1, Math.max(0, volume));
    video.muted = muted;
  });

  // Pause / resume (a paused <video> stops decoding, which is what saves power)
  $effect(() => {
    if (!video) return;
    if (paused) video.pause();
    else void play();
  });

  // Frame-rate cap: copy video frames to a canvas no faster than `fps`.
  $effect(() => {
    const v = video;
    const c = canvas;
    if (!v || !c || fps === 0) return;
    const ctx = c.getContext("2d", { alpha: false })!;
    const interval = 1000 / fps;
    let nextDue = 0;
    let handle = 0;
    let stopped = false;

    const resize = () => {
      const dpr = Math.min(window.devicePixelRatio || 1, 4);
      c.width = Math.floor(window.innerWidth * dpr);
      c.height = Math.floor(window.innerHeight * dpr);
      paint();
    };

    const paint = () => {
      const vw = v.videoWidth;
      const vh = v.videoHeight;
      if (!vw || !vh) return;
      const scale = Math.max(c.width / vw, c.height / vh); // "cover"
      const dw = vw * scale;
      const dh = vh * scale;
      ctx.drawImage(v, (c.width - dw) / 2, (c.height - dh) / 2, dw, dh);
    };

    const onFrame = (now: number) => {
      if (stopped) return;
      if (now >= nextDue - 4) {
        paint();
        // accumulate so e.g. 24 fps from a 30 fps source averages 24, not 15
        nextDue = now - nextDue > interval ? now + interval : nextDue + interval;
      }
      handle = v.requestVideoFrameCallback(onFrame);
    };

    resize();
    window.addEventListener("resize", resize);
    v.addEventListener("loadeddata", paint);
    v.addEventListener("seeked", paint);
    handle = v.requestVideoFrameCallback(onFrame);

    return () => {
      stopped = true;
      v.cancelVideoFrameCallback(handle);
      window.removeEventListener("resize", resize);
      v.removeEventListener("loadeddata", paint);
      v.removeEventListener("seeked", paint);
    };
  });
</script>

<video
  bind:this={video}
  {src}
  class:direct
  loop
  playsinline
  preload="auto"
  onloadeddata={() => {
    if (!paused) void play();
  }}
  onerror={onError}
></video>

{#if !direct}
  <canvas bind:this={canvas}></canvas>
{/if}

<style>
  video {
    position: fixed;
    pointer-events: none;
  }
  /* Capped mode: the element only feeds the canvas. Kept 2px/near-invisible
     (not display:none) so the browser keeps presenting frames to it. */
  video:not(.direct) {
    left: 0;
    top: 0;
    width: 2px;
    height: 2px;
    opacity: 0.01;
  }
  video.direct {
    inset: 0;
    width: 100%;
    height: 100%;
    object-fit: cover;
  }
  canvas {
    position: fixed;
    inset: 0;
    width: 100%;
    height: 100%;
    display: block;
  }
</style>
