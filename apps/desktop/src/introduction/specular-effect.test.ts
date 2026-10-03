import { beforeEach, describe, expect, it, vi } from 'vitest';
import { createSpecularEffect } from './specular-effect';

const glTest = vi.hoisted(() => ({
  fail: false,
  render: vi.fn(),
  remove: vi.fn(),
  deleteProgram: vi.fn(),
  lose: vi.fn(),
  disconnect: vi.fn(),
  options: vi.fn(),
}));
vi.mock('ogl', () => ({
  Renderer: class {
    isWebgl2 = true;
    gl = {
      canvas: document.createElement('canvas'),
      clearColor: vi.fn(),
      deleteProgram: glTest.deleteProgram,
      getExtension: () => ({ loseContext: glTest.lose }),
    };
    constructor(options: unknown) {
      glTest.options(options);
      if (glTest.fail) throw new Error('Unavailable');
    }
    setSize() {}
    render = glTest.render;
  },
  Program: class {
    uniforms: unknown;
    program = {};
    constructor(_gl: unknown, options: { uniforms: unknown }) {
      this.uniforms = options.uniforms;
    }
  },
  Triangle: class {
    remove = glTest.remove;
  },
  Mesh: class {},
}));

let queued: FrameRequestCallback | undefined;
beforeEach(() => {
  glTest.fail = false;
  for (const fn of [
    glTest.render,
    glTest.remove,
    glTest.deleteProgram,
    glTest.lose,
    glTest.disconnect,
    glTest.options,
  ])
    fn.mockClear();
  queued = undefined;
  vi.stubGlobal(
    'requestAnimationFrame',
    vi.fn((fn: FrameRequestCallback) => {
      queued = fn;
      return 1;
    }),
  );
  vi.stubGlobal(
    'cancelAnimationFrame',
    vi.fn(() => {
      queued = undefined;
    }),
  );
  vi.stubGlobal(
    'ResizeObserver',
    class {
      observe() {}
      disconnect = glTest.disconnect;
    },
  );
});
function nodes() {
  const button = document.createElement('button');
  button.textContent = 'Get Started';
  button.getBoundingClientRect = () => ({
    width: 200,
    height: 56,
    left: 0,
    top: 0,
    right: 200,
    bottom: 56,
    x: 0,
    y: 0,
    toJSON: () => ({}),
  });
  const host = document.createElement('span');
  button.append(host);
  return { button, host };
}
function frame(time: number) {
  const fn = queued;
  queued = undefined;
  fn?.(time);
}

describe('specular renderer lifetime', () => {
  it('leaves an ordinary working button when WebGL initialization fails', () => {
    glTest.fail = true;
    const { button, host } = nodes();
    const click = vi.fn();
    button.addEventListener('click', click);
    const dispose = createSpecularEffect(button, host, false);
    expect(host.childElementCount).toBe(0);
    button.click();
    expect(click).toHaveBeenCalledOnce();
    expect(() => dispose()).not.toThrow();
  });

  it('caps DPR/draw rate, stops at rest and disposes GPU/listener resources once', () => {
    vi.stubGlobal('devicePixelRatio', 3);
    const { button, host } = nodes();
    const dispose = createSpecularEffect(button, host, true);
    expect(glTest.options).toHaveBeenCalledWith(
      expect.objectContaining({ dpr: 1.5 }),
    );
    frame(40);
    expect(glTest.render).toHaveBeenCalledOnce();
    expect(queued).toBeUndefined();
    button.dispatchEvent(new Event('focus'));
    frame(50);
    frame(60);
    expect(glTest.render).toHaveBeenCalledOnce();
    frame(80);
    expect(glTest.render).toHaveBeenCalledTimes(2);
    dispose();
    dispose();
    expect(host.childElementCount).toBe(0);
    expect(glTest.disconnect).toHaveBeenCalledOnce();
    expect(glTest.remove).toHaveBeenCalledOnce();
    expect(glTest.deleteProgram).toHaveBeenCalledOnce();
    expect(glTest.lose).toHaveBeenCalledOnce();
    expect(queued).toBeUndefined();
    button.dispatchEvent(new Event('focus'));
    expect(queued).toBeUndefined();
  });

  it('falls back after context loss without affecting the button', () => {
    const { button, host } = nodes();
    const dispose = createSpecularEffect(button, host, true);
    host
      .querySelector('canvas')
      ?.dispatchEvent(new Event('webglcontextlost', { cancelable: true }));
    expect(host.childElementCount).toBe(0);
    expect(button.textContent).toBe('Get Started');
    expect(queued).toBeUndefined();
    dispose();
    expect(glTest.lose).toHaveBeenCalledOnce();
  });
});
