<script lang="ts">
  import { untrack } from "svelte";

  interface Props {
    src: string;
    /** 0 = unlimited: the <video> elements are shown directly. Otherwise frames are copied to a canvas at this rate. */
    fps?: number;
    volume?: number;
    muted?: boolean;
    paused?: boolean;
    /** Playback speed, 1 = normal. */
    speed?: number;
    /** Loop transition in seconds. 0 = seamless hand-off to a pre-loaded copy, > 0 = crossfade. */
    loopFade?: number;
    /** Draw through a canvas with high-quality resampling (less aliasing when the video is scaled down). */
    smooth?: boolean;
    /** cover = fill and crop, contain = fit with bars, fill = stretch. */
    fit?: "cover" | "contain" | "fill";
    onstatus?: (ok: boolean, message?: string) => void;
  }

  let {
    src,
    fps = 0,
    volume = 0.5,
    muted = true,
    paused = false,
    speed = 1,
    loopFade = 0,
    smooth = false,
    fit = "cover",
    onstatus,
  }: Props = $props();

  // Two copies of the same video take turns. While one plays, the other sits
  // paused on its first frame, already decoded. At the end of the clip the
  // standby copy takes over, so looping never waits for a seek + decode (the
  // cause of the stutter with the native `loop` attribute).
  let a = $state<HTMLVideoElement>();
  let b = $state<HTMLVideoElement>();
  let canvas = $state<HTMLCanvasElement>();
  // The elements are shown directly only when no frame limit and no smoothing is wanted.
  const direct = $derived(fps === 0 && !smooth);

  // Player state. Plain variables on purpose: it changes every frame and is
  // never rendered by Svelte.
  let cur = 0; // 0 -> `a` is the active video, 1 -> `b`
  let fading = false; // a hand-off / crossfade is in progress
  let fadeLen = 0; // media seconds the crossfade lasts (0 = instant hand-off)
  let mix = 0; // share of the standby video shown (0..1)
  let forcedMute = false;
  let frameDur = 1 / 30;
  let lastMedia = -1;
  let paintNow: (() => void) | null = null;

  const active = () => (cur === 0 ? a : b)!;
  const standby = () => (cur === 0 ? b : a)!;
  const clamp = (x: number, lo: number, hi: number) => Math.min(hi, Math.max(lo, x));

  function unsupported() {
    return "The webview can't decode this file (unsupported container or codec). H.264/AAC in MP4 and VP9/Opus in WebM are the safest choices.";
  }

  function onError(v: HTMLVideoElement) {
    const err = v.error;
    onstatus?.(false, err?.code === 4 ? unsupported() : `Playback error${err?.message ? `: ${err.message}` : ""}`);
  }

  function applyAudio() {
    if (!a || !b) return;
    const v = clamp(volume, 0, 1);
    const m = muted || forcedMute;
    const x = active();
    const y = standby();
    x.muted = y.muted = m;
    x.volume = v * (1 - mix);
    y.volume = v * mix;
  }

  /** Stacks the videos: the active one below, the incoming one on top at `mix` opacity. */
  function layout() {
    if (!a || !b) return;
    const x = active();
    const y = standby();
    if (!direct) {
      for (const v of [x, y]) {
        v.style.opacity = "";
        v.style.zIndex = "";
      }
      return;
    }
    x.style.opacity = "1";
    x.style.zIndex = "1";
    y.style.opacity = String(mix);
    y.style.zIndex = "2";
  }

  async function play(v: HTMLVideoElement, report: boolean) {
    try {
      await v.play();
      if (report) onstatus?.(true);
    } catch (e) {
      const name = e instanceof DOMException ? e.name : "";
      if (name === "AbortError") return; // a new source replaced this one
      if (name === "NotAllowedError" && !v.muted) {
        // Sound autoplay was refused: keep the picture running, silently.
        forcedMute = true;
        applyAudio();
        try {
          await v.play();
          if (report) onstatus?.(true, "The webview blocked sound autoplay, so the video is playing muted.");
          return;
        } catch {
          /* fall through */
        }
      }
      if (report) onstatus?.(false, name === "NotSupportedError" ? unsupported() : String(e));
    }
  }

  function startTransition() {
    if (fading || !a || !b) return;
    const x = active();
    const y = standby();
    const d = x.duration;
    const rate = x.playbackRate || 1;
    fadeLen = isFinite(d) && d > 0 ? Math.min(Math.max(loopFade, 0) * rate, d / 3) : 0;
    fading = true;
    mix = 0;
    if (Math.abs(y.currentTime) > 0.01) y.currentTime = 0;
    void play(y, false);
  }

  function finishTransition() {
    const old = active();
    cur = 1 - cur;
    fading = false;
    mix = 0;
    lastMedia = -1;
    old.pause();
    try {
      old.currentTime = 0; // becomes the standby: decodes its first frame now
    } catch {
      /* not seekable yet */
    }
    layout();
    applyAudio();
  }

  function tick() {
    if (paused || !a || !b) return;
    const x = active();
    const y = standby();

    if (!fading) {
      const d = x.duration;
      if (!isFinite(d) || d <= 0) return;
      const rate = x.playbackRate || 1;
      const fade = Math.min(Math.max(loopFade, 0) * rate, d / 3);
      // Start the next copy early enough to cover the seam: the whole fade for
      // a crossfade, or about two frames for a seamless hand-off.
      const lead = fade > 0 ? fade : Math.max(0.05, frameDur * 2);
      if (d - x.currentTime <= lead && y.readyState >= 2) startTransition();
      return;
    }

    mix = fadeLen > 0 ? clamp(y.currentTime / fadeLen, 0, 1) : y.currentTime > 0.001 ? 1 : 0;
    layout();
    applyAudio();
    if (mix >= 1 || x.ended) finishTransition();
  }

  function onLoaded(v?: HTMLVideoElement) {
    if (v && v === active() && !paused) void play(v, true);
    paintNow?.();
  }

  function onEnded(v?: HTMLVideoElement) {
    // Safety net: the clip reached its end before `tick` could hand over.
    if (v && v === active() && !fading) startTransition();
  }

  // New source: start over with the first copy in front.
  $effect(() => {
    const x = a;
    const y = b;
    void src;
    if (!x || !y) return;
    untrack(() => {
      cur = 0;
      fading = false;
      mix = 0;
      forcedMute = false;
      frameDur = 1 / 30;
      lastMedia = -1;
      x.disablePictureInPicture = y.disablePictureInPicture = true;
      x.disableRemotePlayback = y.disableRemotePlayback = true;
      layout();
      applyAudio();
    });
  });

  // Sound
  $effect(() => {
    void volume;
    void muted;
    applyAudio();
  });

  // Playback speed. `defaultPlaybackRate` survives a source change.
  $effect(() => {
    if (!a || !b) return;
    const r = clamp(speed, 0.25, 2);
    a.defaultPlaybackRate = b.defaultPlaybackRate = r;
    a.playbackRate = b.playbackRate = r;
  });

  // Direct <-> canvas switch changes how the videos are stacked.
  $effect(() => {
    void direct;
    layout();
  });

  // Redraw the canvas right away when the fit mode changes.
  $effect(() => {
    void fit;
    paintNow?.();
  });

  // Pause / resume (a paused <video> stops decoding, which is what saves power)
  $effect(() => {
    if (!a || !b) return;
    if (paused) {
      a.pause();
      b.pause();
    } else {
      untrack(() => {
        void play(active(), true);
        if (fading) void play(standby(), false);
      });
    }
  });

  // Frame loop: hand-over logic every display frame, canvas painting (when the
  // frame rate is capped) every presented video frame.
  $effect(() => {
    const x = a;
    const y = b;
    if (!x || !y) return;
    const c = canvas; // only exists when the video is drawn through the canvas
    const capped = !!c;
    const ctx = c ? c.getContext("2d", { alpha: false }) : null;
    const interval = fps > 0 ? 1000 / fps : 0; // 0 = paint every video frame
    let nextDue = 0;
    let stopped = false;
    let raf = 0;
    const handles = new Map<HTMLVideoElement, number>();

    const draw = (v: HTMLVideoElement, alpha: number) => {
      if (!ctx || !c) return;
      const vw = v.videoWidth;
      const vh = v.videoHeight;
      if (!vw || !vh) return;
      let dw = c.width;
      let dh = c.height;
      if (fit !== "fill") {
        const scale =
          fit === "contain" ? Math.min(c.width / vw, c.height / vh) : Math.max(c.width / vw, c.height / vh);
        dw = vw * scale;
        dh = vh * scale;
      }
      // Context state is reset whenever the canvas is resized, so set it per draw.
      ctx.imageSmoothingEnabled = true;
      ctx.imageSmoothingQuality = "high";
      ctx.globalAlpha = alpha;
      ctx.drawImage(v, (c.width - dw) / 2, (c.height - dh) / 2, dw, dh);
    };

    const paint = () => {
      if (!ctx || !c) return;
      if (fit !== "cover") {
        ctx.globalAlpha = 1;
        ctx.fillStyle = "#000";
        ctx.fillRect(0, 0, c.width, c.height);
      }
      draw(active(), 1);
      if (fading && mix > 0) draw(standby(), mix);
      ctx.globalAlpha = 1;
    };

    const resize = () => {
      if (!c) return;
      const dpr = Math.min(window.devicePixelRatio || 1, 4);
      c.width = Math.floor(window.innerWidth * dpr);
      c.height = Math.floor(window.innerHeight * dpr);
      paint();
    };

    const watch = (v: HTMLVideoElement) => {
      const cb: VideoFrameRequestCallback = (now, meta) => {
        if (stopped) return;
        if (v === active()) {
          if (lastMedia >= 0 && meta.mediaTime > lastMedia) {
            frameDur = clamp(frameDur * 0.8 + (meta.mediaTime - lastMedia) * 0.2, 0.004, 0.1);
          }
          lastMedia = meta.mediaTime;
        }
        if (capped && now >= nextDue - 4) {
          paint();
          // accumulate so e.g. 24 fps from a 30 fps source averages 24, not 15
          nextDue = now - nextDue > interval ? now + interval : nextDue + interval;
        }
        handles.set(v, v.requestVideoFrameCallback(cb));
      };
      handles.set(v, v.requestVideoFrameCallback(cb));
    };

    const loop = () => {
      raf = requestAnimationFrame(loop);
      tick();
    };

    if (capped) {
      paintNow = paint;
      resize();
      window.addEventListener("resize", resize);
    }
    watch(x);
    watch(y);
    raf = requestAnimationFrame(loop);

    return () => {
      stopped = true;
      cancelAnimationFrame(raf);
      for (const [v, h] of handles) v.cancelVideoFrameCallback(h);
      window.removeEventListener("resize", resize);
      paintNow = null;
    };
  });
</script>

<video
  bind:this={a}
  {src}
  class:direct
  style:object-fit={fit}
  playsinline
  preload="auto"
  onloadeddata={() => onLoaded(a)}
  onseeked={() => paintNow?.()}
  onended={() => onEnded(a)}
  onerror={() => a && onError(a)}
></video>
<video
  bind:this={b}
  {src}
  class:direct
  style:object-fit={fit}
  playsinline
  preload="auto"
  onloadeddata={() => onLoaded(b)}
  onseeked={() => paintNow?.()}
  onended={() => onEnded(b)}
  onerror={() => b && onError(b)}
></video>

{#if !direct}
  <canvas bind:this={canvas}></canvas>
{/if}

<style>
  video {
    position: fixed;
    pointer-events: none;
  }
  /* Capped mode: the elements only feed the canvas. Kept 2px/near-invisible
     (not display:none) so the browser keeps presenting frames to them. */
  video:not(.direct) {
    left: 0;
    top: 0;
    width: 2px;
    height: 2px;
    opacity: 0.01;
  }
  /* Opacity and stacking are set from script (see `layout`). */
  video.direct {
    inset: 0;
    width: 100%;
    height: 100%;
    object-fit: cover;
    opacity: 0;
  }
  canvas {
    position: fixed;
    inset: 0;
    width: 100%;
    height: 100%;
    display: block;
  }
</style>
