import { act, render, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { LiquidEther } from './LiquidEther';
const gpu = vi.hoisted(() => ({
  create: vi.fn(),
  render: vi.fn(),
  resize: vi.fn(),
  pointer: vi.fn(),
  resetPointer: vi.fn(),
  dispose: vi.fn(),
  unavailable: false,
}));
vi.mock('./liquid-simulation', () => ({
  createLiquidSimulation: () => {
    gpu.create();
    if (gpu.unavailable) throw new Error('No WebGL');
    return gpu;
  },
}));
let frames: Map<number, FrameRequestCallback>;
let nextId = 0;
function tick(time: number) {
  act(() => {
    const pending = [...frames.values()];
    frames.clear();
    pending.forEach((callback) => callback(time));
  });
}
beforeEach(() => {
  vi.resetAllMocks();
  gpu.unavailable = false;
  frames = new Map();
  vi.stubGlobal(
    'requestAnimationFrame',
    vi.fn((callback: FrameRequestCallback) => {
      const id = ++nextId;
      frames.set(id, callback);
      return id;
    }),
  );
  vi.stubGlobal(
    'cancelAnimationFrame',
    vi.fn((id: number) => frames.delete(id)),
  );
  vi.stubGlobal(
    'ResizeObserver',
    class {
      observe() {}
      disconnect() {}
    },
  );
});
describe('Liquid Ether lifecycle', () => {
  it('caps drawing, pauses all pending work and releases the simulation', async () => {
    const view = render(<LiquidEther dark active />);
    await waitFor(() => expect(gpu.create).toHaveBeenCalled());
    tick(100);
    tick(110);
    tick(120);
    tick(140);
    expect(gpu.render).toHaveBeenCalledTimes(2);
    view.rerender(<LiquidEther dark active={false} />);
    expect(frames.size).toBe(0);
    tick(200);
    expect(gpu.render).toHaveBeenCalledTimes(2);
    view.rerender(<LiquidEther dark active />);
    expect(frames.size).toBe(1);
    view.unmount();
    expect(frames.size).toBe(0);
    expect(gpu.dispose).toHaveBeenCalledTimes(1);
  });
  it('does not initialize GPU resources after an early unmount during lazy loading', async () => {
    const view = render(<LiquidEther dark active />);
    view.unmount();
    await act(async () => {
      await import('./liquid-simulation');
    });
    expect(gpu.create).not.toHaveBeenCalled();
    expect(frames.size).toBe(0);
  });
  it('leaves no canvas or loop when WebGL cannot start', async () => {
    gpu.unavailable = true;
    const view = render(<LiquidEther dark active />);
    await waitFor(() => expect(gpu.create).toHaveBeenCalled());
    expect(view.container.querySelector('canvas')).toBeNull();
    expect(frames.size).toBe(0);
  });
  it('disposes GPU resources on render failure without repeated retries', async () => {
    const view = render(<LiquidEther dark active />);
    await waitFor(() => expect(gpu.create).toHaveBeenCalled());
    gpu.render.mockImplementationOnce(() => {
      throw new Error('GPU lost');
    });
    tick(100);
    expect(frames.size).toBe(0);
    expect(view.container.querySelector('canvas')).toBeNull();
    expect(gpu.dispose).toHaveBeenCalledTimes(1);
    view.unmount();
    expect(gpu.dispose).toHaveBeenCalledTimes(1);
  });
  it('falls back on context loss and does not restart from a focus change', async () => {
    const view = render(<LiquidEther dark active />);
    await waitFor(() => expect(gpu.create).toHaveBeenCalled());
    const canvas = view.container.querySelector('canvas');
    expect(canvas).not.toBeNull();
    act(() =>
      canvas?.dispatchEvent(
        new Event('webglcontextlost', { cancelable: true }),
      ),
    );
    expect(frames.size).toBe(0);
    expect(view.container.querySelector('canvas')).toBeNull();
    view.rerender(<LiquidEther dark active={false} />);
    view.rerender(<LiquidEther dark active />);
    expect(frames.size).toBe(0);
    expect(gpu.dispose).toHaveBeenCalledTimes(1);
  });
});
