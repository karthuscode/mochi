import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import * as THREE from 'three';
import { createLiquidSimulation } from './liquid-simulation';
const gpu = vi.hoisted(() => ({
  draw: vi.fn(),
  size: vi.fn(),
  dpr: vi.fn(),
  dispose: vi.fn(),
  release: vi.fn(),
  supported: true,
}));
vi.mock('three', async () => {
  const actual = await vi.importActual<typeof THREE>('three');
  return {
    ...actual,
    WebGLRenderer: class {
      extensions = { has: () => gpu.supported };
      debug = { onShaderError: () => {} };
      setPixelRatio = gpu.dpr;
      setSize = gpu.size;
      setClearColor() {}
      setRenderTarget() {}
      clear() {}
      render = gpu.draw;
      dispose = gpu.dispose;
      forceContextLoss = gpu.release;
    },
  };
});
beforeEach(() => {
  vi.clearAllMocks();
  gpu.supported = true;
  vi.stubGlobal('devicePixelRatio', 3);
});
afterEach(() => vi.restoreAllMocks());
describe('bounded Liquid Ether GPU resources', () => {
  it('caps render resolution and simulation buffers, then disposes every allocation', () => {
    const size = vi.spyOn(THREE.WebGLRenderTarget.prototype, 'setSize');
    const targets = vi.spyOn(THREE.WebGLRenderTarget.prototype, 'dispose');
    const materials = vi.spyOn(THREE.RawShaderMaterial.prototype, 'dispose');
    const geometries = vi.spyOn(THREE.PlaneGeometry.prototype, 'dispose');
    const palette = vi.spyOn(THREE.DataTexture.prototype, 'dispose');
    const sim = createLiquidSimulation(document.createElement('canvas'), true);
    expect(gpu.dpr).toHaveBeenCalledWith(1.5);
    sim.resize(3840, 2160);
    const lastSizes = size.mock.calls.slice(-5);
    expect(lastSizes).toHaveLength(5);
    for (const [width, height] of lastSizes) {
      expect(width * height).toBeLessThanOrEqual(262144);
      expect(Math.max(width, height)).toBeLessThanOrEqual(768);
    }
    sim.render(100);
    expect(gpu.draw).toHaveBeenCalledTimes(30);
    expect(targets).toHaveBeenCalledTimes(5); // Resizing frees prior framebuffers.
    targets.mockClear();
    sim.dispose();
    sim.dispose();
    expect(targets).toHaveBeenCalledTimes(5);
    expect(materials).toHaveBeenCalledTimes(6);
    expect(geometries).toHaveBeenCalledTimes(6);
    expect(palette).toHaveBeenCalledTimes(1);
    expect(gpu.dispose).toHaveBeenCalledTimes(1);
    expect(gpu.release).toHaveBeenCalledTimes(1);
    vi.restoreAllMocks();
  });
  it('releases the renderer when float render targets are unsupported', () => {
    gpu.supported = false;
    expect(() =>
      createLiquidSimulation(document.createElement('canvas'), false),
    ).toThrow('Background unavailable');
    expect(gpu.dispose).toHaveBeenCalledTimes(1);
    expect(gpu.release).toHaveBeenCalledTimes(1);
  });
});
