// Adapted from React Bits Liquid Ether (David Haz). See THIRD_PARTY_NOTICES.md.
import * as THREE from 'three';
import {
  face_vert,
  mouse_vert,
  advection_frag,
  color_frag,
  divergence_frag,
  externalForce_frag,
  poisson_frag,
  pressure_frag,
} from './liquid-shaders';

type Disposable = { dispose: () => void };
type Uniforms = Record<
  string,
  { value: number | boolean | THREE.Texture | THREE.Vector2 | THREE.Vector4 }
>;
export type LiquidSimulation = {
  resize: (width: number, height: number) => void;
  pointer: (x: number, y: number, time: number) => void;
  resetPointer: () => void;
  render: (time: number) => void;
  dispose: () => void;
};
const COLORS = ['#5227FF', '#FF9FFC', '#B497CF'];
const DT = 0.014;
const CURSOR_SIZE = 75;
const MOUSE_FORCE = 15;
const ITERATIONS = 25;
const RESOLUTION = 0.5;
const PIXEL_BUDGET = 262144;

export function createLiquidSimulation(
  canvas: HTMLCanvasElement,
  dark: boolean,
): LiquidSimulation {
  const resources = new Set<Disposable>();
  let renderer: THREE.WebGLRenderer | undefined;
  let disposed = false;
  const own = <T extends Disposable>(item: T) => {
    resources.add(item);
    return item;
  };
  const dispose = () => {
    if (disposed) return;
    disposed = true;
    for (const resource of resources) resource.dispose();
    resources.clear();
    renderer?.dispose();
    renderer?.forceContextLoss();
  };
  try {
    renderer = new THREE.WebGLRenderer({
      canvas,
      alpha: true,
      antialias: false,
      premultipliedAlpha: false,
    });
    const gpu = renderer;
    gpu.setPixelRatio(Math.min(window.devicePixelRatio || 1, 1.5));
    gpu.autoClear = false;
    gpu.setClearColor(0x000000, 0);
    if (!gpu.extensions.has('EXT_color_buffer_float'))
      throw new Error('Background unavailable');
    let shaderFailed = false;
    gpu.debug.onShaderError = () => {
      shaderFailed = true;
    };
    const paletteBytes = new Uint8Array(COLORS.length * 4);
    COLORS.forEach((hex, index) => {
      const color = new THREE.Color(hex);
      paletteBytes.set(
        [color.r * 255, color.g * 255, color.b * 255, 255],
        index * 4,
      );
    });
    const palette = own(
      new THREE.DataTexture(paletteBytes, COLORS.length, 1, THREE.RGBAFormat),
    );
    palette.minFilter = palette.magFilter = THREE.LinearFilter;
    palette.wrapS = palette.wrapT = THREE.ClampToEdgeWrapping;
    palette.generateMipmaps = false;
    palette.needsUpdate = true;
    const target = () =>
      own(
        new THREE.WebGLRenderTarget(1, 1, {
          type: THREE.HalfFloatType,
          depthBuffer: false,
          stencilBuffer: false,
          minFilter: THREE.LinearFilter,
          magFilter: THREE.LinearFilter,
          wrapS: THREE.ClampToEdgeWrapping,
          wrapT: THREE.ClampToEdgeWrapping,
        }),
      );
    const velocity0 = target(),
      velocity1 = target(),
      divergence = target();
    const pressure0 = target(),
      pressure1 = target();
    const targets = [velocity0, velocity1, divergence, pressure0, pressure1];
    const px = new THREE.Vector2(1, 1);
    const size = new THREE.Vector2(1, 1);
    function pass<T extends Uniforms>(
      fragment: string,
      uniforms: T,
      mouse = false,
    ) {
      const material = own(
        new THREE.RawShaderMaterial({
          vertexShader: mouse ? mouse_vert : face_vert,
          fragmentShader: fragment,
          uniforms,
          depthTest: false,
          depthWrite: false,
          blending: mouse ? THREE.AdditiveBlending : THREE.NoBlending,
        }),
      );
      const geometry = own(
        new THREE.PlaneGeometry(mouse ? 1 : 2, mouse ? 1 : 2),
      );
      const scene = new THREE.Scene();
      scene.add(new THREE.Mesh(geometry, material));
      const camera = new THREE.Camera();
      return {
        uniforms,
        draw: (output: THREE.WebGLRenderTarget | null) => {
          gpu.setRenderTarget(output);
          gpu.render(scene, camera);
          if (shaderFailed) throw new Error('Background unavailable');
        },
      };
    }
    const advection = pass(advection_frag, {
      boundarySpace: { value: px },
      px: { value: px },
      fboSize: { value: size },
      velocity: { value: velocity0.texture },
      dt: { value: DT },
      isBFECC: { value: true },
    });
    const force = pass(
      externalForce_frag,
      {
        px: { value: px },
        force: { value: new THREE.Vector2() },
        center: { value: new THREE.Vector2() },
        scale: { value: new THREE.Vector2(CURSOR_SIZE, CURSOR_SIZE) },
      },
      true,
    );
    const div = pass(divergence_frag, {
      boundarySpace: { value: px },
      px: { value: px },
      dt: { value: DT },
      velocity: { value: velocity1.texture },
    });
    const poisson = pass(poisson_frag, {
      boundarySpace: { value: px },
      px: { value: px },
      pressure: { value: pressure0.texture },
      divergence: { value: divergence.texture },
    });
    const pressure = pass(pressure_frag, {
      boundarySpace: { value: px },
      px: { value: px },
      dt: { value: DT },
      velocity: { value: velocity1.texture },
      pressure: { value: pressure1.texture },
    });
    const output = pass(color_frag, {
      boundarySpace: { value: new THREE.Vector2() },
      velocity: { value: velocity0.texture },
      palette: { value: palette },
      bgColor: { value: new THREE.Vector4(0, 0, 0, 0) },
      lightMode: { value: !dark },
    });
    const coords = new THREE.Vector2();
    const previousCoords = new THREE.Vector2();
    const userTarget = new THREE.Vector2();
    const takeoverFrom = new THREE.Vector2();
    const autoTarget = new THREE.Vector2();
    const direction = new THREE.Vector2();
    let lastInteraction = -3000;
    let previousTime: number | null = null;
    let takeoverStart = 0;
    let autoSince: number | null = null;
    let userControl = false;
    const pickTarget = () =>
      autoTarget.set(
        (Math.random() * 2 - 1) * 0.8,
        (Math.random() * 2 - 1) * 0.8,
      );
    pickTarget();
    const advancePointer = (time: number) => {
      const delta =
        previousTime === null ? 0 : Math.min((time - previousTime) / 1000, 0.1);
      previousTime = time;
      let automatic = false;
      if (userControl && time - lastInteraction < 3000) {
        autoSince = null;
        const t = Math.min(1, Math.max(0, (time - takeoverStart) / 250));
        coords.copy(takeoverFrom).lerp(userTarget, t * t * (3 - 2 * t));
      } else {
        automatic = true;
        userControl = false;
        autoSince ??= time;
        const t = Math.min(1, Math.max(0, (time - autoSince) / 600));
        direction.subVectors(autoTarget, coords);
        const distance = direction.length();
        if (distance < 0.01) pickTarget();
        else
          coords.addScaledVector(
            direction.normalize(),
            Math.min(distance, 0.5 * delta * t * t * (3 - 2 * t)),
          );
      }
      const strength = (MOUSE_FORCE * (automatic ? 2.2 : 1)) / 2;
      force.uniforms.force.value
        .subVectors(coords, previousCoords)
        .multiplyScalar(strength);
      previousCoords.copy(coords);
      const radiusX = Math.min(0.95, CURSOR_SIZE * px.x + px.x * 2);
      const radiusY = Math.min(0.95, CURSOR_SIZE * px.y + px.y * 2);
      force.uniforms.center.value.set(
        THREE.MathUtils.clamp(coords.x, -1 + radiusX, 1 - radiusX),
        THREE.MathUtils.clamp(coords.y, -1 + radiusY, 1 - radiusY),
      );
    };
    return {
      dispose,
      pointer: (x, y, time) => {
        if (!userControl) {
          takeoverFrom.copy(coords);
          takeoverStart = time;
        }
        userTarget.set(
          THREE.MathUtils.clamp(x, -1, 1),
          THREE.MathUtils.clamp(y, -1, 1),
        );
        userControl = true;
        lastInteraction = time;
      },
      resetPointer: () => {
        previousTime = null;
        previousCoords.copy(coords);
      },
      resize: (width, height) => {
        if (disposed) return;
        gpu.setSize(Math.max(1, width), Math.max(1, height), false);
        let w = Math.max(1, Math.round(width * RESOLUTION));
        let h = Math.max(1, Math.round(height * RESOLUTION));
        const factor = Math.min(
          1,
          768 / Math.max(w, h),
          Math.sqrt(PIXEL_BUDGET / (w * h)),
        );
        w = Math.max(1, Math.floor(w * factor));
        h = Math.max(1, Math.floor(h * factor));
        size.set(w, h);
        px.set(1 / w, 1 / h);
        for (const buffer of targets) {
          buffer.setSize(w, h);
          gpu.setRenderTarget(buffer);
          gpu.clear();
        }
        gpu.setRenderTarget(null);
        gpu.clear();
      },
      render: (time) => {
        if (disposed) return;
        advancePointer(time);
        advection.draw(velocity1);
        force.draw(velocity1);
        div.draw(divergence);
        let source = pressure0,
          destination = pressure1;
        for (let i = 0; i < ITERATIONS; i++) {
          poisson.uniforms.pressure.value = source.texture;
          poisson.draw(destination);
          [source, destination] = [destination, source];
        }
        pressure.uniforms.pressure.value = source.texture;
        pressure.draw(velocity0);
        gpu.setRenderTarget(null);
        gpu.clear();
        output.draw(null);
      },
    };
  } catch (error) {
    dispose();
    throw error;
  }
}
