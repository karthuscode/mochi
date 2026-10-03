// Adapted from React Bits SpecularButton (David Haz). See THIRD_PARTY_NOTICES.md.
import { Mesh, Program, Renderer, Triangle } from 'ogl';
import { FRAG, VERT } from './specular-shaders';

const PAD = 20;
const FRAME_MS = 1000 / 30;

export function createSpecularEffect(
  button: HTMLButtonElement,
  host: HTMLSpanElement,
  dark: boolean,
): () => void {
  let renderer: Renderer | undefined;
  let program: Program | undefined;
  let geometry: Triangle | undefined;
  let observer: ResizeObserver | undefined;
  let frame: number | undefined;
  let disposed = false;
  const dispose = () => {
    if (disposed) return;
    disposed = true;
    if (frame !== undefined) cancelAnimationFrame(frame);
    observer?.disconnect();
    window.removeEventListener('pointermove', pointer);
    button.removeEventListener('focus', focus);
    button.removeEventListener('blur', blur);
    const gl = renderer?.gl;
    if (gl) {
      gl.canvas.removeEventListener('webglcontextlost', lost);
      geometry?.remove();
      if (program) gl.deleteProgram(program.program);
      gl.canvas.remove();
      gl.getExtension('WEBGL_lose_context')?.loseContext();
    }
  };
  let pointerAngle = 2.4;
  let proximity = 0;
  let focused = false;
  let angle = 2.4;
  let bright = 0;
  let lastDraw = 0;
  let mesh: Mesh;
  const draw = (now: number) => {
    frame = undefined;
    if (disposed || !program || !renderer) return;
    if (now - lastDraw >= FRAME_MS) {
      const dt = Math.min((now - lastDraw) / 1000, 0.05);
      lastDraw = now;
      const diff =
        ((pointerAngle - angle + Math.PI * 3) % (Math.PI * 2)) - Math.PI;
      angle += diff * (1 - Math.exp(-dt * 7));
      const target = focused ? 1 : proximity;
      bright += (target - bright) * (1 - Math.exp(-dt * 8));
      program.uniforms.uAngle.value = angle;
      program.uniforms.uIntensity.value = bright;
      try {
        renderer.render({ scene: mesh });
      } catch {
        dispose();
        return;
      }
      // No idle sweep: stop requesting frames once the pointer response settles.
      if (Math.abs(diff) < 0.001 && Math.abs(target - bright) < 0.001) return;
    }
    frame = requestAnimationFrame(draw);
  };
  const requestDraw = () => {
    if (!disposed && frame === undefined) frame = requestAnimationFrame(draw);
  };
  const pointer = (event: PointerEvent) => {
    const rect = button.getBoundingClientRect();
    if (!rect.width || !rect.height) return;
    const dx = Math.max(
      rect.left - event.clientX,
      0,
      event.clientX - rect.right,
    );
    const dy = Math.max(
      rect.top - event.clientY,
      0,
      event.clientY - rect.bottom,
    );
    const distance = Math.hypot(dx, dy);
    const cx = rect.left + rect.width / 2;
    const cy = rect.top + rect.height / 2;
    pointerAngle =
      distance === 0
        ? Math.atan2(2 / rect.height, -2 / rect.width) +
          ((event.clientX - cx) / (rect.width / 2)) * 0.3 +
          ((cy - event.clientY) / (rect.height / 2)) * 0.15
        : Math.atan2(cy - event.clientY, event.clientX - cx);
    const t = Math.max(0, 1 - distance / 250);
    proximity = t * t * (3 - 2 * t);
    if (proximity > 0 || bright > 0.001) requestDraw();
  };
  const focus = () => {
    focused = true;
    requestDraw();
  };
  const blur = () => {
    focused = false;
    requestDraw();
  };
  const lost = (event: Event) => {
    event.preventDefault();
    dispose();
  };
  try {
    const dpr = Math.min(window.devicePixelRatio || 1, 1.5);
    renderer = new Renderer({
      alpha: true,
      premultipliedAlpha: true,
      antialias: true,
      dpr,
      webgl: 2,
    });
    const gl = renderer.gl;
    if (!renderer.isWebgl2) {
      dispose();
      return dispose;
    }
    gl.clearColor(0, 0, 0, 0);
    geometry = new Triangle(gl);
    program = new Program(gl, {
      vertex: VERT,
      fragment: FRAG,
      transparent: true,
      depthTest: false,
      depthWrite: false,
      uniforms: {
        uCenter: { value: [0, 0] },
        uHalfSize: { value: [1, 1] },
        uRadius: { value: 18 * dpr },
        uAngle: { value: angle },
        uPx: { value: dpr },
        uLineColor: { value: dark ? [1, 0.86, 0.78] : [0.7, 0.25, 0.12] },
        uBaseColor: { value: dark ? [0.4, 0.4, 0.4] : [0.5, 0.5, 0.5] },
        uIntensity: { value: 0 },
        uShineSize: { value: (10 * Math.PI) / 180 },
        uShineFade: { value: (40 * Math.PI) / 180 },
        uThickness: { value: dpr },
        uBaseWidth: { value: dpr },
      },
    });
    mesh = new Mesh(gl, { geometry, program });
    host.appendChild(gl.canvas);
    const resize = () => {
      if (disposed || !renderer || !program) return;
      const { width, height } = button.getBoundingClientRect();
      if (width <= 0 || height <= 0) return;
      try {
        renderer.setSize(width + PAD * 2, height + PAD * 2);
        program.uniforms.uCenter.value = [
          (PAD + width / 2) * dpr,
          (PAD + height / 2) * dpr,
        ];
        program.uniforms.uHalfSize.value = [
          (width / 2) * dpr,
          (height / 2) * dpr,
        ];
        program.uniforms.uRadius.value =
          Math.min(18, Math.min(width, height) / 2) * dpr;
        requestDraw();
      } catch {
        dispose();
      }
    };
    observer = new ResizeObserver(resize);
    observer.observe(button);
    gl.canvas.addEventListener('webglcontextlost', lost);
    window.addEventListener('pointermove', pointer, { passive: true });
    button.addEventListener('focus', focus);
    button.addEventListener('blur', blur);
    resize();
  } catch {
    dispose();
  }
  return dispose;
}
