import { LocalCapture } from './LocalCapture';
import { LearningSettings } from './Learning';
import { CompanionControls } from './CompanionControls';
import { AppFrame } from '@mochi/ui';
import { isTauri } from '@tauri-apps/api/core';
import { useState } from 'react';
import { AuraBackground } from './background/AuraBackground';
import { useMediaPreference } from './background/preferences';
import type { Appearance } from './background/preferences';
import { getAppInfo } from './native/app-info';
import mark from '../../../docs/design/assets/mochi/final/mark.svg';

export function App() {
  const native = isTauri();
  const [page, setPage] = useState<'sessions' | 'settings'>('sessions');
  const [appearance, setAppearance] = useState<Appearance>('system');
  const [animated, setAnimated] = useState(true);
  const systemDark = useMediaPreference('(prefers-color-scheme: dark)');
  const reducedMotion = useMediaPreference('(prefers-reduced-motion: reduce)');
  const dark = appearance === 'dark' || (appearance === 'system' && systemDark);
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
    <AppFrame
      className={`appearance-${appearance} theme-${dark ? 'dark' : 'light'}`}
    >
      <AuraBackground dark={dark} animated={animated} />
      <a className="skip-link" href="#workspace">
        Skip to content
      </a>
      <header className="glass-header">
        <div className="brand">
          <img src={mark} alt="" aria-hidden="true" />
          <span>mochi</span>
        </div>
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
        <div className="header-status">
          <span className="privacy-dot" aria-hidden="true" /> Local by default
        </div>
      </header>
      <div className="workspace" id="workspace" tabIndex={-1}>
        <header className="workspace-header">
          <div>
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
              <label className="checkbox background-toggle">
                <input
                  type="checkbox"
                  checked={animated && !reducedMotion}
                  disabled={reducedMotion}
                  onChange={(event) => setAnimated(event.target.checked)}
                />
                Animated background
              </label>
              <p className="muted">
                {reducedMotion
                  ? 'Animation is off to follow your Mac’s Reduce Motion setting.'
                  : 'Appearance changes apply to this window.'}
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
            <details className="panel about-preview">
              <summary id="about-title">About & diagnostics</summary>
              <p className="muted">Personal CLI preview · 0.1.0</p>
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
            </details>
          </div>
        )}
      </div>
    </AppFrame>
  );
}
