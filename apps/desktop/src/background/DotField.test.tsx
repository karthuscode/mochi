import { act, fireEvent, render } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { DotField } from './DotField';

const drawing = {
  clearRect: vi.fn(),
  beginPath: vi.fn(),
  moveTo: vi.fn(),
  arc: vi.fn(),
  fill: vi.fn(),
  setTransform: vi.fn(),
  createLinearGradient: vi.fn(() => ({ addColorStop: vi.fn() })),
};
let frames: Map<number, FrameRequestCallback>;
let sequence = 0;
let clock = 0;
let width = 600;
let height = 400;
const disconnect = vi.fn();
function tick(time: number) {
  clock = time;
  act(() => {
    const pending = [...frames.values()];
    frames.clear();
    pending.forEach((callback) => callback(time));
  });
}
function move(x = 300, y = 200) {
  fireEvent(window, new MouseEvent('pointermove', { clientX: x, clientY: y }));
}
beforeEach(() => {
  vi.restoreAllMocks();
  Object.values(drawing).forEach((method) => {
    if (vi.isMockFunction(method)) method.mockClear();
  });
  disconnect.mockClear();
  width = 600;
  height = 400;
  clock = 0;
  frames = new Map();
  vi.spyOn(performance, 'now').mockImplementation(() => clock);
  vi.spyOn(HTMLElement.prototype, 'getBoundingClientRect').mockImplementation(
    () => ({
      x: 0,
      y: 0,
      top: 0,
      left: 0,
      right: width,
      bottom: height,
      width,
      height,
      toJSON: () => ({}),
    }),
  );
  vi.spyOn(HTMLCanvasElement.prototype, 'getContext').mockReturnValue(
    drawing as unknown as CanvasRenderingContext2D,
  );
  vi.stubGlobal(
    'requestAnimationFrame',
    vi.fn((callback: FrameRequestCallback) => {
      const id = ++sequence;
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
      disconnect() {
        disconnect();
      }
    },
  );
});

describe('DotField decoration lifecycle', () => {
  it('draws once at rest, bounds the frame rate and stops after the cursor settles', () => {
    const view = render(<DotField dark animated active />);
    expect(view.container.firstChild).toHaveAttribute('data-rendered', 'true');
    expect(drawing.fill).toHaveBeenCalledTimes(1);
    expect(frames.size).toBe(0);
    move();
    move(320, 200);
    expect(frames.size).toBe(1);
    tick(100);
    tick(110);
    tick(120);
    tick(140);
    expect(drawing.fill).toHaveBeenCalledTimes(3);
    tick(5100);
    expect(frames.size).toBe(0);
    expect(view.container.querySelector('circle')).toHaveStyle({
      opacity: '0',
    });
  });
  it('keeps drawing coordinates finite even when the cursor coincides with a dot', () => {
    render(<DotField dark animated active />);
    const [x, y] = drawing.arc.mock.calls[0] as number[];
    move(x, y);
    tick(100);
    expect(
      drawing.arc.mock.calls.every((args) => args.every(Number.isFinite)),
    ).toBe(true);
  });
  it('pauses without allocating canvas resources when inactive or animation is off', () => {
    const view = render(<DotField dark animated={false} active />);
    expect(HTMLCanvasElement.prototype.getContext).not.toHaveBeenCalled();
    view.rerender(<DotField dark animated active={false} />);
    expect(HTMLCanvasElement.prototype.getContext).not.toHaveBeenCalled();
    view.rerender(<DotField dark animated active />);
    move();
    view.rerender(<DotField dark animated active={false} />);
    expect(frames.size).toBe(0);
    expect(disconnect).toHaveBeenCalledTimes(1);
    expect(view.container.firstChild).not.toHaveAttribute('data-rendered');
    move();
    expect(frames.size).toBe(0);
    view.rerender(<DotField dark animated active />);
    move();
    view.unmount();
    expect(frames.size).toBe(0);
    expect(disconnect).toHaveBeenCalledTimes(2);
    move();
    expect(frames.size).toBe(0);
  });
  it('caps dot count and backbuffer size on a large high-density display', () => {
    width = 7680;
    height = 4320;
    vi.stubGlobal('devicePixelRatio', 3);
    const view = render(<DotField dark animated active />);
    const canvas = view.container.querySelector('canvas')!;
    expect(canvas.width * canvas.height).toBeLessThanOrEqual(4_000_000);
    expect(Math.max(canvas.width, canvas.height)).toBeLessThanOrEqual(4096);
    expect(drawing.arc.mock.calls.length).toBeLessThanOrEqual(10_000);
    expect(drawing.arc.mock.calls.length).toBeGreaterThan(0);
  });
  it('releases pending work when the viewport collapses and redraws on resize', () => {
    const view = render(<DotField dark animated active />);
    move();
    width = 0;
    fireEvent(window, new Event('resize'));
    expect(frames.size).toBe(0);
    expect(view.container.firstChild).not.toHaveAttribute('data-rendered');
    move(0, 200);
    expect(frames.size).toBe(0);
    width = 600;
    fireEvent(window, new Event('resize'));
    expect(view.container.firstChild).toHaveAttribute('data-rendered', 'true');
    expect(frames.size).toBe(0);
  });
  it('keeps the static decoration when a canvas context is unavailable', () => {
    vi.spyOn(HTMLCanvasElement.prototype, 'getContext').mockReturnValue(null);
    const view = render(<DotField dark animated active />);
    expect(view.container.firstChild).not.toHaveAttribute('data-rendered');
    move();
    expect(frames.size).toBe(0);
  });
  it('falls back to static dots and removes listeners after a drawing failure', () => {
    const view = render(<DotField dark animated active />);
    drawing.fill.mockImplementationOnce(() => {
      throw new Error('Canvas lost');
    });
    move();
    tick(100);
    expect(view.container.firstChild).not.toHaveAttribute('data-rendered');
    expect(frames.size).toBe(0);
    expect(disconnect).toHaveBeenCalledTimes(1);
    move();
    tick(200);
    expect(frames.size).toBe(0);
    view.unmount();
    expect(disconnect).toHaveBeenCalledTimes(1);
  });
  it('releases resources on context loss', () => {
    const view = render(<DotField dark animated active />);
    move();
    fireEvent(
      view.container.querySelector('canvas')!,
      new Event('contextlost'),
    );
    expect(frames.size).toBe(0);
    expect(view.container.firstChild).not.toHaveAttribute('data-rendered');
    expect(disconnect).toHaveBeenCalledTimes(1);
  });
});
