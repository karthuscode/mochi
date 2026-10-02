// Adapted from React Bits Liquid Ether (David Haz). See THIRD_PARTY_NOTICES.md.
import { useEffect, useRef } from 'react';
import type { LiquidSimulation } from './liquid-simulation';

const FRAME_MS = 1000 / 30;

export function LiquidEther({
  dark,
  active,
}: {
  dark: boolean;
  active: boolean;
}) {
  const container = useRef<HTMLDivElement>(null);
  const requestedActive = useRef(false);
  const controls = useRef<{ start: () => void; stop: () => void } | null>(null);
  useEffect(() => {
    const host = container.current;
    if (!host) return;
    const canvas = document.createElement('canvas');
    let simulation: LiquidSimulation | undefined;
    let observer: ResizeObserver | undefined;
    let frame: number | null = null;
    let stopped = true;
    let failed = false;
    let disposed = false;
    let lastDraw = 0;
    const stop = () => {
      stopped = true;
      if (frame !== null) cancelAnimationFrame(frame);
      frame = null;
      lastDraw = 0;
      simulation?.resetPointer();
    };
    const pointer = (event: PointerEvent) => {
      if (stopped || failed) return;
      const bounds = host.getBoundingClientRect();
      if (bounds.width <= 0 || bounds.height <= 0) return;
      simulation?.pointer(
        ((event.clientX - bounds.left) / bounds.width) * 2 - 1,
        1 - ((event.clientY - bounds.top) / bounds.height) * 2,
        performance.now(),
      );
    };
    const lost = (event: Event) => {
      event.preventDefault();
      fail();
    };
    const dispose = () => {
      if (disposed) return;
      disposed = true;
      stop();
      observer?.disconnect();
      window.removeEventListener('pointermove', pointer);
      canvas.removeEventListener('webglcontextlost', lost);
      simulation?.dispose();
      canvas.remove();
      controls.current = null;
    };
    const fail = () => {
      failed = true;
      dispose();
    };
    const initialize = async () => {
      try {
        const { createLiquidSimulation } = await import('./liquid-simulation');
        if (disposed) return;
        host.appendChild(canvas);
        simulation = createLiquidSimulation(canvas, dark);
        const resize = () => {
          if (disposed) return;
          try {
            const bounds = host.getBoundingClientRect();
            simulation?.resize(bounds.width, bounds.height);
          } catch {
            fail();
          }
        };
        resize();
        if (!disposed) {
          observer = new ResizeObserver(resize);
          observer.observe(host);
          window.addEventListener('pointermove', pointer, { passive: true });
          canvas.addEventListener('webglcontextlost', lost);
          const loop = (time: number) => {
            frame = null;
            if (stopped || failed) return;
            if (time - lastDraw >= FRAME_MS) {
              try {
                simulation?.render(time);
              } catch {
                fail();
              }
              lastDraw = time;
            }
            if (!stopped && !failed) frame = requestAnimationFrame(loop);
          };
          controls.current = {
            stop,
            start: () => {
              if (!stopped || failed) return;
              stopped = false;
              frame = requestAnimationFrame(loop);
            },
          };
          if (requestedActive.current) controls.current?.start();
        }
      } catch {
        fail();
      }
    };
    void initialize();
    return dispose;
  }, [dark]);
  useEffect(() => {
    requestedActive.current = active;
    if (active) controls.current?.start();
    else controls.current?.stop();
  }, [active, dark]);
  return <div className="liquid-ether" ref={container} />;
}
