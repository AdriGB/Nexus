import { state } from "../state";
import { TERRAIN, BASE_TILE } from "../constants";

const miniCanvas = document.getElementById("minimap-canvas") as HTMLCanvasElement;
const miniCtx = miniCanvas.getContext("2d")!;

function hslToRgb(h: number, s: number, l: number): [number, number, number] {
  h /= 360;
  s /= 100;
  l /= 100;
  let r: number, g: number, b: number;
  if (s === 0) {
    r = g = b = l;
  } else {
    const hue2rgb = (p: number, q: number, t: number) => {
      if (t < 0) t += 1;
      if (t > 1) t -= 1;
      if (t < 1 / 6) return p + (q - p) * 6 * t;
      if (t < 1 / 2) return q;
      if (t < 2 / 3) return p + (q - p) * (2 / 3 - t) * 6;
      return p;
    };
    const q = l < 0.5 ? l * (1 + s) : l + s - l * s;
    const p = 2 * l - q;
    r = hue2rgb(p, q, h + 1 / 3);
    g = hue2rgb(p, q, h);
    b = hue2rgb(p, q, h - 1 / 3);
  }
  return [Math.round(r * 255), Math.round(g * 255), Math.round(b * 255)];
}

export function renderMinimap(): void {
  if (!state.world) return;

  const mw = miniCanvas.width;
  const mh = miniCanvas.height;
  const scaleX = mw / state.worldW;
  const scaleY = mh / state.worldH;
  const data = state.world.get_tile_data(0, 0, state.worldW, state.worldH);
  const imageData = miniCtx.createImageData(mw, mh);

  // Background
  for (let i = 0; i < imageData.data.length; i += 4) {
    imageData.data[i] = 7;
    imageData.data[i + 1] = 8;
    imageData.data[i + 2] = 12;
    imageData.data[i + 3] = 255;
  }

  for (let y = 0; y < state.worldH; y++) {
    for (let x = 0; x < state.worldW; x++) {
      const ti = (y * state.worldW + x) * 4;
      const t = TERRAIN[data[ti]] ?? TERRAIN[0];
      const [r, g, b] = hslToRgb(t.h, t.s, t.l);
      const px = Math.floor(x * scaleX);
      const py = Math.floor(y * scaleY);
      for (let dy = 0; dy < Math.ceil(scaleY) && py + dy < mh; dy++) {
        for (let dx = 0; dx < Math.ceil(scaleX) && px + dx < mw; dx++) {
          const pi = ((py + dy) * mw + (px + dx)) * 4;
          imageData.data[pi] = r;
          imageData.data[pi + 1] = g;
          imageData.data[pi + 2] = b;
          imageData.data[pi + 3] = 255;
        }
      }
    }
  }

  state.minimapImageData = imageData;
  miniCtx.putImageData(imageData, 0, 0);
}

export function drawMinimapViewport(): void {
  if (!state.minimapImageData) return;
  const mw = miniCanvas.width;
  const mh = miniCanvas.height;
  miniCtx.putImageData(state.minimapImageData, 0, 0);

  const rx = (state.panX / (BASE_TILE * state.zoom) / state.worldW) * mw;
  const ry = (state.panY / (BASE_TILE * state.zoom) / state.worldH) * mh;
  const rw = (state.cssW / (BASE_TILE * state.zoom) / state.worldW) * mw;
  const rh = (state.cssH / (BASE_TILE * state.zoom) / state.worldH) * mh;

  miniCtx.strokeStyle = "rgba(201,168,76,0.8)";
  miniCtx.lineWidth = 1.5;
  miniCtx.strokeRect(
    Math.max(0, rx),
    Math.max(0, ry),
    Math.min(mw - rx, rw),
    Math.min(mh - ry, rh)
  );
}
