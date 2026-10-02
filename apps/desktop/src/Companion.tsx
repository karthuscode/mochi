import { useRef, useState } from 'react';
import {
  hideCompanion,
  openMochi,
  startCompanionDrag,
} from './native/companion';
import character from '../../../docs/design/assets/mochi/final/character-idle.png';

export function Companion() {
  const [error, setError] = useState(false);
  const [missingImage, setMissingImage] = useState(false);
  const start = useRef<{ x: number; y: number } | null>(null);
  const dragged = useRef(false);
  async function activate(keyboard: boolean) {
    if (dragged.current && !keyboard) {
      dragged.current = false;
      return;
    }
    dragged.current = false;
    try {
      await openMochi();
      setError(false);
    } catch {
      setError(true);
    }
  }
  return (
    <main className="companion-surface">
      <button
        type="button"
        className="companion-character"
        aria-label="Open mochi"
        title="Drag to move · Click to open mochi"
        onPointerDown={(event) => {
          if (event.button !== 0) return;
          start.current = { x: event.clientX, y: event.clientY };
          dragged.current = false;
        }}
        onPointerMove={(event) => {
          if (!start.current || dragged.current || (event.buttons & 1) === 0)
            return;
          if (
            Math.hypot(
              event.clientX - start.current.x,
              event.clientY - start.current.y,
            ) < 4
          )
            return;
          dragged.current = true;
          start.current = null;
          void startCompanionDrag().catch(() => setError(true));
        }}
        onPointerUp={() => {
          start.current = null;
        }}
        onPointerCancel={() => {
          start.current = null;
        }}
        onClick={(event) => void activate(event.detail === 0)}
        onKeyDown={(event) => {
          if (event.key === 'Escape') {
            void hideCompanion().catch(() => setError(true));
          }
        }}
      >
        {missingImage ? (
          <span>Open mochi</span>
        ) : (
          <img
            src={character}
            alt=""
            draggable={false}
            onError={() => setMissingImage(true)}
          />
        )}
      </button>
      {error && (
        <p className="companion-error" role="alert">
          Use mochi’s Dock icon.
        </p>
      )}
    </main>
  );
}
