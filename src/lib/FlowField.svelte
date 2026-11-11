<script lang="ts">
  import { onMount } from "svelte";

  interface Props {
    count?: number;
    fps?: number;
    paused?: boolean;
  }

  let { count = 900, fps = 30, paused = false }: Props = $props();

  interface Particle {
    x: number;
    y: number;
    age: number;
    life: number;
    hue: number;
  }

  let canvas: HTMLCanvasElement;

  onMount(() => {
    const ctx = canvas.getContext("2d")!;
    const BG = "#12102b";
    let w = 0;
    let h = 0;
    let raf = 0;
    let last = 0;
    let t = 0;
    let particles: Particle[] = [];

    const spawn = (p?: Particle): Particle => {
      const q = p ?? ({} as Particle);
      q.x = Math.random() * w;
      q.y = Math.random() * h;
      q.age = 0;
      q.life = 120 + Math.random() * 240;
      q.hue = 220 + Math.random() * 120; // blue -> magenta
      return q;
    };

    const resize = () => {
      const dpr = Math.min(window.devicePixelRatio || 1, 2);
      w = window.innerWidth;
      h = window.innerHeight;
      canvas.width = Math.floor(w * dpr);
      canvas.height = Math.floor(h * dpr);
      ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
      ctx.fillStyle = BG;
      ctx.fillRect(0, 0, w, h);
      particles = Array.from({ length: count }, () => spawn());
    };

    const angle = (x: number, y: number) =>
      Math.sin(x * 0.0021 + t * 0.00018) * 2.2 +
      Math.cos(y * 0.0027 - t * 0.00013) * 2.2 +
      Math.sin((x + y) * 0.0009) * 1.6;

    const frame = (now: number) => {
      raf = requestAnimationFrame(frame);
      if (paused) return;
      if (now - last < 1000 / fps) return;
      last = now;
      t = now;

      // fade previous frame to leave trails
      ctx.fillStyle = "rgba(18, 16, 43, 0.06)";
      ctx.fillRect(0, 0, w, h);

      ctx.lineWidth = 1.2;
      for (const p of particles) {
        const a = angle(p.x, p.y);
        const nx = p.x + Math.cos(a) * 1.8;
        const ny = p.y + Math.sin(a) * 1.8;
        const fade = Math.sin((p.age / p.life) * Math.PI);
        ctx.strokeStyle = `hsla(${p.hue}, 85%, 68%, ${0.55 * fade})`;
        ctx.beginPath();
        ctx.moveTo(p.x, p.y);
        ctx.lineTo(nx, ny);
        ctx.stroke();
        p.x = nx;
        p.y = ny;
        p.age++;
        if (p.age >= p.life || p.x < -10 || p.x > w + 10 || p.y < -10 || p.y > h + 10) {
          spawn(p);
        }
      }
    };

    resize();
    window.addEventListener("resize", resize);
    raf = requestAnimationFrame(frame);

    return () => {
      cancelAnimationFrame(raf);
      window.removeEventListener("resize", resize);
    };
  });
</script>

<canvas bind:this={canvas}></canvas>

<style>
  canvas {
    position: fixed;
    inset: 0;
    width: 100%;
    height: 100%;
    display: block;
  }
</style>
