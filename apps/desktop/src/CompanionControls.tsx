import { useState } from 'react';
import { setCompanionVisible } from './native/companion';

export function CompanionControls() {
  const [busy, setBusy] = useState(false);
  const [status, setStatus] = useState('');
  async function change(visible: boolean) {
    setBusy(true);
    try {
      const result = await setCompanionVisible(visible);
      setStatus(
        result.visible
          ? 'Desktop mochi is visible.'
          : 'Desktop mochi is hidden.',
      );
    } catch {
      setStatus('Desktop mochi is unavailable. Restart mochi and try again.');
    } finally {
      setBusy(false);
    }
  }
  return (
    <section
      className="panel companion-controls"
      aria-labelledby="desktop-mochi-title"
    >
      <h2 id="desktop-mochi-title">Desktop mochi</h2>
      <p>Place mochi where you want. Drag to move it; click to open the app.</p>
      <p className="muted">
        Static character trial. Hidden on startup; placement lasts for this run.
        Animation and saved placement are still ahead.
      </p>
      <div className="actions">
        <button type="button" disabled={busy} onClick={() => void change(true)}>
          Show desktop mochi
        </button>
        <button
          type="button"
          disabled={busy}
          onClick={() => void change(false)}
        >
          Hide desktop mochi
        </button>
      </div>
      <p role="status">{status}</p>
    </section>
  );
}
