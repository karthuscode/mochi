import { LocalCapture } from './LocalCapture';
import { AppFrame } from '@mochi/ui';
import { isTauri } from '@tauri-apps/api/core';
import { useState } from 'react';
import { getAppInfo } from './native/app-info';

export function App() {
  const [status, setStatus] = useState('');
  const [checking, setChecking] = useState(false);

  async function checkConnection() {
    setChecking(true);
    try {
      const info = await getAppInfo();
      setStatus(`Desktop connected · ${info.name} ${info.version}`);
    } catch {
      setStatus('Desktop connection failed. Restart Mochi and try again.');
    } finally {
      setChecking(false);
    }
  }

  return (
    <AppFrame>
      <h1>Mochi</h1>
      <p>V1 development build</p>
      {isTauri() ? (
        <button
          type="button"
          disabled={checking}
          onClick={() => void checkConnection()}
        >
          {checking ? 'Checking…' : 'Check desktop connection'}
        </button>
      ) : (
        <p>Desktop connection is available in the Tauri app.</p>
      )}
      <p role="status">{status}</p>
      {isTauri() && <LocalCapture />}
    </AppFrame>
  );
}
