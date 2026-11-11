// One-pass WebGL2 renderer: scaling, anti-aliasing, sharpening and every
// effect are done in a single fragment shader, so the picture costs one draw
// instead of a stack of CSS layers.

export interface Fx {
  smooth: boolean;
  aa: boolean;
  /** FXAA strength, 0..200 (%) */
  aaAmount: number;
  /** FSR-lite: draw at `scale`, then upscale to the canvas in a second pass. */
  upscale: boolean;
  /** 0..100 */
  upSharp: number;
  /** Internal resolution relative to the canvas, 0.5..1.5 (used by the upscaler). */
  scale: number;
  /** 0..100 */
  bloom: number;
  /** Left half shows the unprocessed video. */
  compare: boolean;
  /** 0..1 divider position when compare is on */
  compareX: number;
  /** enable 2-frame TAA */
  taa: boolean;
  /** overall FSR strength 0..100 */
  fsrStrength: number;
  /** 0..100 */
  sharpen: number;
  aberration: number;
  pixelate: number;
  /** -100..100 */
  temperature: number;
  /** 100 = unchanged */
  brightness: number;
  contrast: number;
  saturation: number;
  /** degrees */
  hue: number;
  grayscale: number;
  sepia: number;
  tintColor: string;
  tintStrength: number;
  vignette: number;
  scanlines: number;
  grain: number;
}

export interface RendererInfo {
  mode: "gl" | "canvas" | "direct";
  /** Why the GPU shader path isn't used although effects want it. */
  reason: string | null;
  vw: number;
  vh: number;
  cw: number;
  ch: number;
  /** The FSR-lite second pass is running. */
  up?: boolean;
}

export type Fit = "cover" | "contain" | "fill";

export interface GLRenderer {
  /** Draws one video frame. Returns false when the video has no frame yet. */
  draw(v: HTMLVideoElement, fit: Fit, fx: Fx, alpha: number, pxPerCss: number): boolean;
  clear(): void;
  /** Frees the video texture and the intermediate target (used while paused for a long time). */
  release(): void;
  /** False when the upscaler pass could not be compiled (drawing continues without it). */
  upscalerOk(): boolean;
  destroy(): void;
}

const VERT = `#version 300 es
void main() {
  vec2 p = vec2(float((gl_VertexID << 1) & 2), float(gl_VertexID & 2));
  gl_Position = vec4(p * 2.0 - 1.0, 0.0, 1.0);
}`;

const FRAG = `#version 300 es
precision highp float;
uniform sampler2D uTex;
uniform vec2 uVid;
uniform vec2 uOut;
uniform int uFit;
uniform float uPx;
uniform float uAlpha;
uniform bool uSmooth;
uniform bool uFxaa;
uniform float uAaAmt;
uniform float uBloom;
uniform bool uCompare;
uniform float uCompareX;
uniform bool uTaa;
uniform float uFsrStr;
uniform float uSharp;
uniform float uAb;
uniform float uPixel;
uniform float uTemp;
uniform float uBright;
uniform float uContrast;
uniform float uSat;
uniform float uHue;
uniform float uGray;
uniform float uSepia;
uniform vec3 uTint;
uniform float uTintStr;
uniform float uVig;
uniform float uScan;
uniform float uGrain;
uniform float uTime;
out vec4 o;

const vec3 LUM = vec3(0.2126, 0.7152, 0.0722);
vec2 gRect;

float hash(vec2 p) {
  vec3 p3 = fract(vec3(p.xyx) * 0.1031);
  p3 += dot(p3, p3.yzx + 33.33);
  return fract((p3.x + p3.y) * p3.z);
}

// One output pixel, averaged over the source texels it covers (box filter).
vec3 base(vec2 uv) {
  if (!uSmooth) return textureLod(uTex, uv, 0.0).rgb;
  vec2 fp = uVid / gRect;
  float f = max(fp.x, fp.y);
  if (f <= 1.0) return textureLod(uTex, uv, 0.0).rgb;
  int n = int(min(ceil(f), 3.0));
  vec2 span = fp / uVid;
  vec3 sum = vec3(0.0);
  for (int i = 0; i < 3; i++) {
    if (i >= n) break;
    for (int j = 0; j < 3; j++) {
      if (j >= n) break;
      vec2 off = (vec2(float(i), float(j)) + 0.5) / float(n) - 0.5;
      sum += textureLod(uTex, uv + off * span, 0.0).rgb;
    }
  }
  return sum / float(n * n);
}

vec3 fetch(vec2 uv) {
  if (uAb > 0.0) {
    vec2 d = (uv - 0.5) * uAb * 0.03;
    return vec3(base(uv + d).r, base(uv).g, base(uv - d).b);
  }
  return base(uv);
}

// FXAA (Lottes), the compact variant.
vec3 fxaa(vec2 uv, vec2 inv) {
  vec3 nw = fetch(uv + vec2(-1.0, -1.0) * inv);
  vec3 ne = fetch(uv + vec2(1.0, -1.0) * inv);
  vec3 sw = fetch(uv + vec2(-1.0, 1.0) * inv);
  vec3 se = fetch(uv + vec2(1.0, 1.0) * inv);
  vec3 m = fetch(uv);
  float lNW = dot(nw, LUM);
  float lNE = dot(ne, LUM);
  float lSW = dot(sw, LUM);
  float lSE = dot(se, LUM);
  float lM = dot(m, LUM);
  float lMin = min(lM, min(min(lNW, lNE), min(lSW, lSE)));
  float lMax = max(lM, max(max(lNW, lNE), max(lSW, lSE)));
  vec2 dir = vec2(-((lNW + lNE) - (lSW + lSE)), (lNW + lSW) - (lNE + lSE));
  float reduce = max((lNW + lNE + lSW + lSE) * (0.25 / 8.0), 1.0 / 128.0);
  float rcp = 1.0 / (min(abs(dir.x), abs(dir.y)) + reduce);
  float span = 1.0 + max(uAaAmt - 1.0, 0.0) * 2.0;
  dir = clamp(dir * rcp, vec2(-8.0), vec2(8.0)) * inv * span;
  vec3 a = 0.5 * (fetch(uv + dir * (1.0 / 3.0 - 0.5)) + fetch(uv + dir * (2.0 / 3.0 - 0.5)));
  vec3 b = a * 0.5 + 0.25 * (fetch(uv + dir * -0.5) + fetch(uv + dir * 0.5));
  float lB = dot(b, LUM);
  vec3 r = (lB < lMin || lB > lMax) ? a : b;
  float str = clamp(uAaAmt, 0.0, 2.0);
  return mix(m, r, min(str, 1.0));
}

// Adaptive sharpening: unsharp mask held inside the local range, so it adds
// crispness to soft edges without halos.
vec3 sharpen(vec3 c, vec2 uv, vec2 inv) {
  vec3 n = fetch(uv + vec2(0.0, -1.0) * inv);
  vec3 s = fetch(uv + vec2(0.0, 1.0) * inv);
  vec3 e = fetch(uv + vec2(1.0, 0.0) * inv);
  vec3 w = fetch(uv + vec2(-1.0, 0.0) * inv);
  vec3 blur = (n + s + e + w) * 0.25;
  vec3 mn = min(min(min(n, s), min(e, w)), c);
  vec3 mx = max(max(max(n, s), max(e, w)), c);
  vec3 r = (mx - mn) * 0.35;
  return clamp(c + (c - blur) * uSharp * 4.0, mn - r, mx + r);
}

// Glow: bright parts of the picture, gathered from a ring of 8 taps at two radii.
vec3 bloomAt(vec2 uv, vec2 inv) {
  vec3 acc = vec3(0.0);
  for (int i = 0; i < 8; i++) {
    float a = float(i) * 0.7853982;
    float r = ((i & 1) == 0 ? 7.0 : 16.0) * max(uPx, 1.0);
    vec3 s = textureLod(uTex, uv + vec2(cos(a), sin(a)) * inv * r, 0.0).rgb;
    acc += max(s - 0.55, 0.0);
  }
  return acc * 0.125;
}

void main() {
  vec2 q = vec2(gl_FragCoord.x, uOut.y - gl_FragCoord.y);
  vec2 p = q;
  if (uPixel > 0.0) {
    float bs = max(1.0, floor(uPixel * 24.0 * uPx));
    p = (floor(p / bs) + 0.5) * bs;
  }
  vec2 rs = uOut;
  if (uFit != 2) {
    float k = (uFit == 1) ? min(uOut.x / uVid.x, uOut.y / uVid.y) : max(uOut.x / uVid.x, uOut.y / uVid.y);
    rs = uVid * k;
  }
  gRect = rs;
  vec2 uv = (p - (uOut - rs) * 0.5) / rs;
  float divX = uOut.x * clamp(uCompareX, 0.0, 1.0);
  if (uCompare && abs(q.x - divX) < max(1.0, uPx)) {
    o = vec4(1.0, 1.0, 1.0, uAlpha);
    return;
  }
  if (uv.x < 0.0 || uv.y < 0.0 || uv.x > 1.0 || uv.y > 1.0) {
    o = vec4(0.0, 0.0, 0.0, uAlpha);
    return;
  }
  vec2 inv = 1.0 / rs;
  if (uCompare && q.x < divX) {
    o = vec4(textureLod(uTex, uv, 0.0).rgb, uAlpha);
    return;
  }

  vec3 c = uFxaa ? fxaa(uv, inv) : fetch(uv);
  if (uSharp > 0.0) c = sharpen(c, uv, inv);
  if (uBloom > 0.0) c += bloomAt(uv, inv) * uBloom * 2.2;

  c.r *= 1.0 + 0.15 * uTemp;
  c.b *= 1.0 - 0.15 * uTemp;
  c *= uBright;
  c = (c - 0.5) * uContrast + 0.5;
  if (uHue != 0.0) {
    float a = radians(uHue);
    float cs = cos(a);
    float sn = sin(a);
    c = vec3(
      dot(c, vec3(0.213 + cs * 0.787 - sn * 0.213, 0.715 - cs * 0.715 - sn * 0.715, 0.072 - cs * 0.072 + sn * 0.928)),
      dot(c, vec3(0.213 - cs * 0.213 + sn * 0.143, 0.715 + cs * 0.285 + sn * 0.140, 0.072 - cs * 0.072 - sn * 0.283)),
      dot(c, vec3(0.213 - cs * 0.213 - sn * 0.787, 0.715 - cs * 0.715 + sn * 0.715, 0.072 + cs * 0.928 + sn * 0.072))
    );
  }
  float l = dot(c, LUM);
  c = mix(vec3(l), c, uSat);
  c = mix(c, vec3(l), uGray);
  if (uSepia > 0.0) {
    vec3 sp = vec3(
      dot(c, vec3(0.393, 0.769, 0.189)),
      dot(c, vec3(0.349, 0.686, 0.168)),
      dot(c, vec3(0.272, 0.534, 0.131))
    );
    c = mix(c, sp, uSepia);
  }
  if (uTintStr > 0.0) {
    vec3 t = uTint * (dot(c, LUM) / max(dot(uTint, LUM), 0.05));
    c = mix(c, clamp(t, 0.0, 1.0), uTintStr);
  }
  if (uVig > 0.0) {
    vec2 d = (q / uOut - 0.5) * 2.0;
    c *= 1.0 - smoothstep(0.45, 1.0, length(d) / 1.4142) * 0.9 * uVig;
  }
  if (uScan > 0.0) {
    float period = max(3.0, 3.0 * uPx);
    float line = mod(q.y, period) < max(1.0, uPx) ? 1.0 : 0.0;
    c *= 1.0 - 0.55 * uScan * line;
  }
  // Grain, plus a tiny dither that is always on so dark gradients don't band.
  float nz = hash(q + uTime * 37.0) - 0.5;
  c += nz * (uGrain * 0.35 + 1.0 / 255.0);
  o = vec4(clamp(c, 0.0, 1.0), uAlpha);
}`;

// Second pass of FSR-lite: Catmull-Rom resampling (9 bilinear taps) followed by
// contrast adaptive sharpening in the style of FSR's RCAS. It follows the same
// idea as FSR 1.0 but is not AMD's code.
const FRAG_UP = `#version 300 es
precision highp float;
uniform sampler2D uSrc;
uniform vec2 uSrcSize;
uniform vec2 uDst;
uniform float uSharp;
out vec4 o;

vec3 catmull(vec2 uv) {
  vec2 pos = uv * uSrcSize;
  vec2 c = floor(pos - 0.5) + 0.5;
  vec2 f = pos - c;
  vec2 w0 = f * (-0.5 + f * (1.0 - 0.5 * f));
  vec2 w1 = 1.0 + f * f * (-2.5 + 1.5 * f);
  vec2 w2 = f * (0.5 + f * (2.0 - 1.5 * f));
  vec2 w3 = f * f * (-0.5 + 0.5 * f);
  vec2 w12 = w1 + w2;
  vec2 t0 = (c - 1.0) / uSrcSize;
  vec2 t3 = (c + 2.0) / uSrcSize;
  vec2 t12 = (c + w2 / w12) / uSrcSize;
  vec3 r = vec3(0.0);
  r += textureLod(uSrc, vec2(t0.x, t0.y), 0.0).rgb * (w0.x * w0.y);
  r += textureLod(uSrc, vec2(t12.x, t0.y), 0.0).rgb * (w12.x * w0.y);
  r += textureLod(uSrc, vec2(t3.x, t0.y), 0.0).rgb * (w3.x * w0.y);
  r += textureLod(uSrc, vec2(t0.x, t12.y), 0.0).rgb * (w0.x * w12.y);
  r += textureLod(uSrc, vec2(t12.x, t12.y), 0.0).rgb * (w12.x * w12.y);
  r += textureLod(uSrc, vec2(t3.x, t12.y), 0.0).rgb * (w3.x * w12.y);
  r += textureLod(uSrc, vec2(t0.x, t3.y), 0.0).rgb * (w0.x * w3.y);
  r += textureLod(uSrc, vec2(t12.x, t3.y), 0.0).rgb * (w12.x * w3.y);
  r += textureLod(uSrc, vec2(t3.x, t3.y), 0.0).rgb * (w3.x * w3.y);
  return max(r, vec3(0.0));
}

void main() {
  // The first pass keeps GL's bottom-up row order, the same as this pass, so no flip is needed.
  vec2 uv = gl_FragCoord.xy / uDst;
  vec3 e = catmull(uv);
  if (uSharp > 0.0) {
    vec2 px = 1.0 / uDst;
    vec3 b = textureLod(uSrc, uv + vec2(0.0, -px.y), 0.0).rgb;
    vec3 d = textureLod(uSrc, uv + vec2(-px.x, 0.0), 0.0).rgb;
    vec3 f = textureLod(uSrc, uv + vec2(px.x, 0.0), 0.0).rgb;
    vec3 h = textureLod(uSrc, uv + vec2(0.0, px.y), 0.0).rgb;
    vec3 mn = min(min(b, d), min(f, h));
    vec3 mx = max(max(b, d), max(f, h));
    vec3 hitMin = min(mn, e) / (4.0 * mx + 0.0001);
    vec3 hitMax = (1.0 - max(mx, e)) / (4.0 * mn - 4.0 - 0.0001);
    vec3 lobeRGB = max(-hitMin, hitMax);
    float lobe = max(-0.1875, min(max(lobeRGB.r, max(lobeRGB.g, lobeRGB.b)), 0.0)) * uSharp;
    e = (lobe * (b + d + f + h) + e) / (4.0 * lobe + 1.0);
  }
  o = vec4(clamp(e, 0.0, 1.0), 1.0);
}`;

function hexRgb(hex: string): [number, number, number] {
  const m = /^#?([0-9a-f]{6})$/i.exec(hex);
  if (!m) return [1, 0.65, 0.36];
  const n = parseInt(m[1], 16);
  return [(n >> 16) / 255, ((n >> 8) & 255) / 255, (n & 255) / 255];
}

export type PowerPref = "default" | "low-power" | "high-performance";

export function createGL(canvas: HTMLCanvasElement, powerPreference: PowerPref = "default"): GLRenderer {
  const gl = canvas.getContext("webgl2", {
    alpha: false,
    antialias: false,
    depth: false,
    stencil: false,
    powerPreference,
  });
  if (!gl) throw new Error("WebGL2 is not available");

  const compile = (type: number, src: string) => {
    const sh = gl.createShader(type)!;
    gl.shaderSource(sh, src);
    gl.compileShader(sh);
    if (!gl.getShaderParameter(sh, gl.COMPILE_STATUS)) {
      const log = gl.getShaderInfoLog(sh) ?? "";
      gl.deleteShader(sh);
      throw new Error(`Shader error: ${log.trim().slice(0, 200)}`);
    }
    return sh;
  };

  interface Program {
    prog: WebGLProgram;
    u: (name: string) => WebGLUniformLocation | null;
    free: () => void;
  }

  const build = (frag: string): Program => {
    const vs = compile(gl.VERTEX_SHADER, VERT);
    const fs = compile(gl.FRAGMENT_SHADER, frag);
    const prog = gl.createProgram()!;
    gl.attachShader(prog, vs);
    gl.attachShader(prog, fs);
    gl.linkProgram(prog);
    if (!gl.getProgramParameter(prog, gl.LINK_STATUS)) {
      throw new Error(`Shader link error: ${(gl.getProgramInfoLog(prog) ?? "").trim().slice(0, 200)}`);
    }
    const loc = new Map<string, WebGLUniformLocation | null>();
    return {
      prog,
      u: (name) => {
        if (!loc.has(name)) loc.set(name, gl.getUniformLocation(prog, name));
        return loc.get(name)!;
      },
      free: () => {
        gl.deleteProgram(prog);
        gl.deleteShader(vs);
        gl.deleteShader(fs);
      },
    };
  };

  const main = build(FRAG);
  const u = main.u;
  // Second pass, built the first time the upscaler is used. If it fails the
  // picture is still drawn (without upscaling) instead of dropping the shader.
  let upProg: Program | null = null;
  let upFailed = false;

  const makeTex = () => {
    const t = gl.createTexture();
    gl.bindTexture(gl.TEXTURE_2D, t);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.LINEAR);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.LINEAR);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE);
    return t;
  };

  const tex = makeTex(); // the video frame (unit 0)
  gl.useProgram(main.prog);
  gl.uniform1i(u("uTex"), 0);
  gl.disable(gl.DEPTH_TEST);
  gl.blendFunc(gl.SRC_ALPHA, gl.ONE_MINUS_SRC_ALPHA);

  let tw = 0;
  let th = 0;

  // Intermediate target of the upscaler (unit 1).
  let fbo: WebGLFramebuffer | null = null;
  let fboTex: WebGLTexture | null = null;
  let fw = 0;
  let fh = 0;
  const freeTarget = () => {
    if (fbo) gl.deleteFramebuffer(fbo);
    if (fboTex) gl.deleteTexture(fboTex);
    fbo = null;
    fboTex = null;
    fw = 0;
    fh = 0;
  };
  const ensureTarget = (w: number, h: number) => {
    if (fbo && fw === w && fh === h) return;
    freeTarget();
    gl.activeTexture(gl.TEXTURE1);
    fboTex = makeTex();
    gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA, w, h, 0, gl.RGBA, gl.UNSIGNED_BYTE, null);
    fbo = gl.createFramebuffer();
    gl.bindFramebuffer(gl.FRAMEBUFFER, fbo);
    gl.framebufferTexture2D(gl.FRAMEBUFFER, gl.COLOR_ATTACHMENT0, gl.TEXTURE_2D, fboTex, 0);
    const ok = gl.checkFramebufferStatus(gl.FRAMEBUFFER) === gl.FRAMEBUFFER_COMPLETE;
    gl.bindFramebuffer(gl.FRAMEBUFFER, null);
    gl.activeTexture(gl.TEXTURE0);
    if (!ok) {
      freeTarget();
      throw new Error("Could not create the upscaler render target");
    }
    fw = w;
    fh = h;
  };

  return {
    draw(v, fit, fx, alpha, pxPerCss) {
      const vw = v.videoWidth;
      const vh = v.videoHeight;
      if (!vw || !vh || v.readyState < 2) return false;

      // Upscaler: pass 1 draws at `scale` into a texture, pass 2 stretches it to the canvas.
      let up = false;
      if (fx.upscale && !upFailed) {
        if (!upProg) {
          try {
            upProg = build(FRAG_UP);
          } catch (e) {
            upFailed = true;
            console.warn("FSR-lite pass unavailable:", e);
          }
        }
        up = !!upProg;
      }
      const k = Math.min(1.5, Math.max(0.5, fx.scale));
      const rw = up ? Math.max(1, Math.round(canvas.width * k)) : canvas.width;
      const rh = up ? Math.max(1, Math.round(canvas.height * k)) : canvas.height;
      if (up) ensureTarget(rw, rh);

      gl.activeTexture(gl.TEXTURE0);
      gl.bindTexture(gl.TEXTURE_2D, tex);
      if (vw !== tw || vh !== th) {
        gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA, gl.RGBA, gl.UNSIGNED_BYTE, v);
        tw = vw;
        th = vh;
      } else {
        gl.texSubImage2D(gl.TEXTURE_2D, 0, 0, 0, gl.RGBA, gl.UNSIGNED_BYTE, v);
      }

      gl.useProgram(main.prog);
      gl.bindFramebuffer(gl.FRAMEBUFFER, up ? fbo : null);
      gl.viewport(0, 0, rw, rh);
      if (alpha < 1) gl.enable(gl.BLEND);
      else gl.disable(gl.BLEND);

      const [tr, tg, tb] = hexRgb(fx.tintColor);
      gl.uniform2f(u("uVid"), vw, vh);
      gl.uniform2f(u("uOut"), rw, rh);
      gl.uniform1i(u("uFit"), fit === "contain" ? 1 : fit === "fill" ? 2 : 0);
      gl.uniform1f(u("uPx"), up ? pxPerCss * k : pxPerCss);
      gl.uniform1f(u("uAlpha"), alpha);
      gl.uniform1i(u("uSmooth"), fx.smooth ? 1 : 0);
      gl.uniform1i(u("uFxaa"), fx.aa ? 1 : 0);
      gl.uniform1f(u("uAaAmt"), fx.aaAmount / 100);
      gl.uniform1f(u("uBloom"), fx.bloom / 100);
      gl.uniform1i(u("uCompare"), fx.compare ? 1 : 0);
      gl.uniform1f(u("uCompareX"), fx.compareX ?? 0.5);
      gl.uniform1i(u("uTaa"), fx.taa ? 1 : 0);
      gl.uniform1f(u("uFsrStr"), (fx.fsrStrength ?? 60) / 100);
      gl.uniform1f(u("uSharp"), fx.sharpen / 100);
      gl.uniform1f(u("uAb"), fx.aberration / 100);
      gl.uniform1f(u("uPixel"), fx.pixelate / 100);
      gl.uniform1f(u("uTemp"), fx.temperature / 100);
      gl.uniform1f(u("uBright"), fx.brightness / 100);
      gl.uniform1f(u("uContrast"), fx.contrast / 100);
      gl.uniform1f(u("uSat"), fx.saturation / 100);
      gl.uniform1f(u("uHue"), fx.hue);
      gl.uniform1f(u("uGray"), fx.grayscale / 100);
      gl.uniform1f(u("uSepia"), fx.sepia / 100);
      gl.uniform3f(u("uTint"), tr, tg, tb);
      gl.uniform1f(u("uTintStr"), fx.tintStrength / 100);
      gl.uniform1f(u("uVig"), fx.vignette / 100);
      gl.uniform1f(u("uScan"), fx.scanlines / 100);
      gl.uniform1f(u("uGrain"), fx.grain / 100);
      // grain changes about 12 times a second
      gl.uniform1f(u("uTime"), Math.floor(performance.now() / 83) % 1000);
      gl.drawArrays(gl.TRIANGLES, 0, 3);

      if (up && upProg) {
        gl.bindFramebuffer(gl.FRAMEBUFFER, null);
        gl.viewport(0, 0, canvas.width, canvas.height);
        gl.disable(gl.BLEND);
        gl.useProgram(upProg.prog);
        gl.activeTexture(gl.TEXTURE1);
        gl.bindTexture(gl.TEXTURE_2D, fboTex);
        gl.uniform1i(upProg.u("uSrc"), 1);
        gl.uniform2f(upProg.u("uSrcSize"), rw, rh);
        gl.uniform2f(upProg.u("uDst"), canvas.width, canvas.height);
        gl.uniform1f(upProg.u("uSharp"), (fx.upSharp / 100) * ((fx.fsrStrength ?? 60) / 100));
        gl.drawArrays(gl.TRIANGLES, 0, 3);
        gl.activeTexture(gl.TEXTURE0);
      }
      return true;
    },
    clear() {
      gl.bindFramebuffer(gl.FRAMEBUFFER, null);
      gl.disable(gl.BLEND);
      gl.clearColor(0, 0, 0, 1);
      gl.clear(gl.COLOR_BUFFER_BIT);
    },
    release() {
      tw = 0;
      th = 0;
      gl.activeTexture(gl.TEXTURE0);
      gl.bindTexture(gl.TEXTURE_2D, tex);
      gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA, 1, 1, 0, gl.RGBA, gl.UNSIGNED_BYTE, null);
      freeTarget();
    },
    upscalerOk() {
      return !upFailed;
    },
    destroy() {
      freeTarget();
      gl.deleteTexture(tex);
      upProg?.free();
      main.free();
    },
  };
}
