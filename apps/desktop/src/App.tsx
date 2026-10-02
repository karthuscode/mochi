import { LocalCapture } from './LocalCapture';
import { LearningSettings } from './Learning';
import { CompanionControls } from './CompanionControls';
import { AppFrame } from '@mochi/ui';
import { isTauri } from '@tauri-apps/api/core';
import { useState } from 'react';
import { getAppInfo } from './native/app-info';
import mark from '../../../docs/design/assets/mochi/final/mark.svg';

export function App() {
  const native = isTauri();
  const [page, setPage] = useState<'sessions' | 'settings'>('sessions');
  const [appearance, setAppearance] = useState<'system' | 'light' | 'dark'>(
    'system',
  );
  const [status, setStatus] = useState('');
  const [checking, setChecking] = useState(false);

  async function checkConnection() {
    setChecking(true);
    try {
      const info = await getAppInfo();
      setStatus(`Desktop connected · mochi ${info.version}`);
    } catch {
      setStatus('Desktop connection failed. Restart mochi and try again.');
    } finally {
      setChecking(false);
    }
  }

  return (
    <AppFrame className={`appearance-${appearance}`}>
      <a className="skip-link" href="#workspace">
        Skip to content
      </a>
      <aside className="sidebar">
        <div className="brand">
          <img src={mark} alt="" aria-hidden="true" />
          <span>mochi</span>
        </div>
        <p className="brand-caption">A little more understanding.</p>
        <nav aria-label="Main navigation">
          <button
            aria-current={page === 'sessions' ? 'page' : undefined}
            onClick={() => setPage('sessions')}
          >
            <span aria-hidden="true">▤</span> Sessions
          </button>
          <button
            aria-current={page === 'settings' ? 'page' : undefined}
            onClick={() => setPage('settings')}
          >
            <span aria-hidden="true">⚙</span> Settings
          </button>
        </nav>
        <div className="sidebar-note">
          <span className="privacy-dot" aria-hidden="true" /> Local by default
          <p>
            Your coding stays on this Mac until you approve an analysis request.
          </p>
        </div>
        <span className="build-label">Personal CLI preview · 0.1.0</span>
      </aside>
      <div className="workspace" id="workspace" tabIndex={-1}>
        <header className="workspace-header">
          <div>
            <p className="eyebrow">Your learning companion</p>
            <h1>{page === 'sessions' ? 'Sessions' : 'Settings'}</h1>
          </div>
          <span className="local-badge">
            {native ? 'On your Mac' : 'Appearance preview'}
          </span>
        </header>
        {!native && (
          <p className="preview-notice" role="status">
            Appearance preview only. Open the desktop app to connect a project,
            capture sessions or use analysis. No data is loaded here.
          </p>
        )}
        <div hidden={page !== 'sessions'}>
          <LocalCapture enabled={native} />
        </div>
        {page === 'settings' && (
          <div className="settings-layout">
            <div className="settings-intro">
              <h2>Make yourself at home.</h2>
              <p>
                Your key, your projects, your choice of what leaves this Mac.
              </p>
            </div>
            <section className="panel" aria-labelledby="appearance-title">
              <h2 id="appearance-title">Appearance</h2>
              <fieldset className="appearance-options">
                <legend>Window theme</legend>
                {(['system', 'light', 'dark'] as const).map((theme) => (
                  <label key={theme}>
                    <input
                      type="radio"
                      name="appearance"
                      value={theme}
                      checked={appearance === theme}
                      onChange={() => setAppearance(theme)}
                    />
                    {theme.charAt(0).toUpperCase() + theme.slice(1)}
                  </label>
                ))}
              </fieldset>
              <p className="muted">
                Applies while this window is open. System follows your Mac’s
                appearance.
              </p>
            </section>
            {native ? (
              <LearningSettings />
            ) : (
              <section className="panel">
                <h2>Learning settings</h2>
                <p>
                  Keychain and remote permission controls are available only in
                  the desktop app. Remote analysis starts off and each request
                  needs your approval.
                </p>
              </section>
            )}
            {native && <CompanionControls />}
            <section className="panel" aria-labelledby="about-title">
              <h2 id="about-title">About this preview</h2>
              <p>
                This personal Codex CLI build captures approved work locally and
                offers explanations with a short self-check. Full lessons,
                challenges and spaced review are still ahead.
              </p>
              <p className="muted">
                Local storage uses account permissions, without
                application-level encryption. Deleting local data cannot recall
                requests already sent to a provider.
              </p>
              <button
                disabled={!native || checking}
                onClick={() => void checkConnection()}
              >
                {checking ? 'Checking…' : 'Check desktop connection'}
              </button>
              <p role="status">{status}</p>
            </section>
          </div>
        )}
      </div>
    </AppFrame>
  );
}
