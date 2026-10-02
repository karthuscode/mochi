import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import { CompanionControls } from './CompanionControls';
import mark from '../../../docs/design/assets/mochi/final/mark.svg';
import character from '../../../docs/design/assets/mochi/final/character-idle.png';
import './companion-preview.css';

const root = document.getElementById('root');
if (root === null) throw new Error('Companion preview root is missing');
createRoot(root).render(
  <StrictMode>
    <main className="trial-main">
      <div className="trial-brand">
        <img src={mark} alt="" aria-hidden="true" />
        <span>mochi</span>
      </div>
      <div className="trial-layout">
        <div>
          <p className="trial-eyebrow">Your desktop companion</p>
          <h1>
            A little mochi.
            <br />
            Right beside you.
          </h1>
          <p className="trial-description">
            A quiet place for your learning companion. Move it to your favorite
            corner, then click it whenever you want to open mochi.
          </p>
          <CompanionControls />
          <p className="trial-note">
            Static character preview. Close this window, then click desktop
            mochi to bring it back. No session tracking or AI analysis runs in
            this preview.
          </p>
        </div>
        <img
          className="trial-hero"
          src={character}
          alt="Signal, mochi’s orange and graphite robot"
        />
      </div>
    </main>
  </StrictMode>,
);
