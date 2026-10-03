// Adapted from the user-supplied React Bits DotField (David Haz).
// Default bulge variant; see THIRD_PARTY_NOTICES.md.
import { memo, useEffect, useId, useRef } from 'react';
import './DotField.css';

const STEP = 15.5;
const RADIUS = 0.75;
const CURSOR_RADIUS = 500;
const BULGE = 67;
const FRAME_MS = 1000 / 30;
const MAX_DOTS = 10_000;
const MAX_PIXELS = 4_000_000;
interface Dot {
  ax: number;
  ay: number;
  x: number;
  y: number;
}

export const DotField = memo(function DotField({
  dark,
  animated,
  active,
}: {
  dark: boolean;
  animated: boolean;
  active: boolean;
}) {
  const hostRef = useRef<HTMLDivElement>(null);
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const glowRef = useRef<SVGCircleElement>(null);
  const glowId = `dot-field-${useId().replace(/:/g, '')}`;
  useEffect(() => {
    if (!animated || !active) return;
    const hostNode = hostRef.current;
    const canvasNode = canvasRef.current;
    const glowNode = glowRef.current;
    if (!hostNode || !canvasNode || !glowNode) return;
    const host = hostNode;
    const canvas = canvasNode;
    const glow = glowNode;
    let frame: number | null = null;
    let observer: ResizeObserver | undefined;
    let disposed = false;
    let width = 0;
    let height = 0;
    let dots: Dot[] = [];
    let gradient: CanvasGradient | null = null;
    let lastDraw = -Infinity;
    let lastInput = -Infinity;
    let engagement = 0;
    let speed = 0;
    const mouse = { x: -9999, y: -9999, seen: false };
    let context: CanvasRenderingContext2D | null = null;
    const reset = () => {
      engagement = speed = 0;
      mouse.seen = false;
      for (const dot of dots) {
        dot.x = dot.ax;
        dot.y = dot.ay;
      }
      glow.style.opacity = '0';
    };
    const dispose = () => {
      if (disposed) return;
      disposed = true;
      if (frame !== null) cancelAnimationFrame(frame);
      frame = null;
      observer?.disconnect();
      window.removeEventListener('pointermove', pointer);
      window.removeEventListener('pointerout', leave);
      window.removeEventListener('resize', resize);
      canvas.removeEventListener('contextlost', dispose);
      host.removeAttribute('data-rendered');
      glow.style.opacity = '0';
      canvas.width = canvas.height = 1;
      dots = [];
    };
    function paint(moving: boolean) {
      if (!context || !gradient) return false;
      context.clearRect(0, 0, width, height);
      context.fillStyle = gradient;
      context.beginPath();
      let unsettled = false;
      for (const d of dots) {
        const dx = mouse.x - d.ax;
        const dy = mouse.y - d.ay;
        const distance = Math.hypot(dx, dy);
        const push =
          moving && mouse.seen && distance < CURSOR_RADIUS
            ? (1 - distance / CURSOR_RADIUS) ** 2 * BULGE * engagement
            : 0;
        // The pointer can coincide with a dot; never divide by zero.
        const ux = distance > 0.001 ? dx / distance : 1;
        const uy = distance > 0.001 ? dy / distance : 0;
        d.x += (d.ax - ux * push - d.x) * 0.15;
        d.y += (d.ay - uy * push - d.y) * 0.15;
        if (Math.abs(d.x - d.ax) + Math.abs(d.y - d.ay) > 0.1) unsettled = true;
        context.moveTo(d.x + RADIUS, d.y);
        context.arc(d.x, d.y, RADIUS, 0, Math.PI * 2);
      }
      context.fill();
      glow.setAttribute('cx', String(mouse.x));
      glow.setAttribute('cy', String(mouse.y));
      glow.style.opacity = String(engagement * 0.7);
      return unsettled;
    }
    function loop(time: number) {
      frame = null;
      if (disposed) return;
      if (time - lastDraw < FRAME_MS) {
        frame = requestAnimationFrame(loop);
        return;
      }
      lastDraw = time;
      try {
        speed *= 0.78;
        engagement += (Math.min(speed / 5, 1) - engagement) * 0.12;
        if (time - lastInput > 5000) reset();
        const unsettled = paint(true);
        if (speed > 0.01 || engagement > 0.002 || unsettled)
          frame = requestAnimationFrame(loop);
        else {
          reset();
          paint(false);
        }
      } catch {
        dispose();
      }
    }
    function pointer(event: PointerEvent) {
      if (disposed || !width || !height || event.pointerType === 'touch')
        return;
      const rect = host.getBoundingClientRect();
      const x = event.clientX - rect.left;
      const y = event.clientY - rect.top;
      if (
        !Number.isFinite(x) ||
        !Number.isFinite(y) ||
        x < 0 ||
        y < 0 ||
        x > width ||
        y > height
      )
        return;
      speed = mouse.seen
        ? Math.min(120, Math.hypot(x - mouse.x, y - mouse.y))
        : 5;
      mouse.x = x;
      mouse.y = y;
      mouse.seen = true;
      lastInput = performance.now();
      if (frame === null) frame = requestAnimationFrame(loop);
    }
    function leave(event: PointerEvent) {
      if (event.relatedTarget !== null) return;
      mouse.seen = false;
      speed = 0;
    }
    function resize() {
      if (disposed || !context) return;
      try {
        const bounds = host.getBoundingClientRect();
        width = Number.isFinite(bounds.width) ? Math.max(0, bounds.width) : 0;
        height = Number.isFinite(bounds.height)
          ? Math.max(0, bounds.height)
          : 0;
        if (!width || !height) {
          if (frame !== null) cancelAnimationFrame(frame);
          frame = null;
          reset();
          dots = [];
          gradient = null;
          canvas.width = canvas.height = 1;
          host.removeAttribute('data-rendered');
          return;
        }
        const dpr = Math.min(
          window.devicePixelRatio || 1,
          1.5,
          Math.sqrt(MAX_PIXELS / (width * height)),
          4096 / width,
          4096 / height,
        );
        canvas.width = Math.max(1, Math.floor(width * dpr));
        canvas.height = Math.max(1, Math.floor(height * dpr));
        context.setTransform(dpr, 0, 0, dpr, 0, 0);
        const step = Math.max(STEP, Math.sqrt((width * height) / MAX_DOTS));
        const cols = Math.floor(width / step),
          rows = Math.floor(height / step);
        const padX = (width % step) / 2,
          padY = (height % step) / 2;
        dots = [];
        for (let row = 0; row < rows; row++)
          for (let col = 0; col < cols; col++) {
            const ax = padX + col * step + step / 2,
              ay = padY + row * step + step / 2;
            dots.push({ ax, ay, x: ax, y: ay });
          }
        gradient = context.createLinearGradient(0, 0, width, height);
        gradient.addColorStop(
          0,
          dark ? 'rgba(168,85,247,0.35)' : 'rgba(119,87,153,0.28)',
        );
        gradient.addColorStop(
          1,
          dark ? 'rgba(180,151,207,0.25)' : 'rgba(136,117,155,0.20)',
        );
        reset();
        paint(false);
        host.setAttribute('data-rendered', 'true');
      } catch {
        dispose();
      }
    }
    try {
      context = canvas.getContext('2d', { alpha: true });
      if (!context) return;
      resize();
      if (!disposed) {
        observer = new ResizeObserver(resize);
        observer.observe(host);
        canvas.addEventListener('contextlost', dispose);
        window.addEventListener('resize', resize);
        window.addEventListener('pointermove', pointer, { passive: true });
        window.addEventListener('pointerout', leave, { passive: true });
      }
    } catch {
      dispose();
    }
    return dispose;
  }, [dark, animated, active]);
  return (
    <div className="dot-field" ref={hostRef} aria-hidden="true">
      <canvas ref={canvasRef} />
      <svg focusable="false" aria-hidden="true">
        <defs>
          <radialGradient id={glowId}>
            <stop offset="0%" stopColor={dark ? '#120F17' : '#eee9f7'} />
            <stop offset="100%" stopColor="transparent" />
          </radialGradient>
        </defs>
        <circle
          ref={glowRef}
          cx="-9999"
          cy="-9999"
          r="160"
          fill={`url(#${glowId})`}
          style={{ opacity: 0 }}
        />
      </svg>
    </div>
  );
});
