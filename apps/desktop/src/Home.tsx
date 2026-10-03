import { useEffect, useRef, useState } from 'react';
import character from '../../../docs/design/assets/mochi/final/character-idle.png';
import { HomeBento } from './HomeBento';
import type { LocalWorkspace, SessionNavigation } from './useLocalWorkspace';
import { BlurText } from './introduction/BlurText';
import { SpecularButton } from './introduction/SpecularButton';
import { SpotlightCard } from './introduction/SpotlightCard';
import { useWindowActive } from './background/preferences';

const slides = [
  {
    title: 'Your work, remembered.',
    text: 'Keep building in your usual tool. Come back to the moments that matter.',
    cards: [
      {
        icon: '⌘',
        title: 'Connect a project',
        text: 'You choose the folder and when local tracking starts.',
        status: 'Available',
      },
      {
        icon: '▤',
        title: 'Revisit your session',
        text: 'Follow recorded requests, actions and code context. Gaps stay visible.',
        status: 'Available',
      },
      {
        icon: '◇',
        title: 'Keep control',
        text: 'History stays on your Mac. You decide what gets sent for analysis.',
        status: 'Available',
      },
    ],
  },
  {
    title: 'Understand the why.',
    text: 'Turn your own project into something you can explain.',
    cards: [
      {
        icon: '≡',
        title: 'A clearer recap',
        text: 'Switch between quick bullet points and a more detailed session summary.',
        status: 'Planned',
      },
      {
        icon: '</>',
        title: 'Code that makes sense',
        text: 'Explore explanations linked to the code and evidence from your work.',
        status: 'Preview',
      },
      {
        icon: '✦',
        title: 'Focus on useful ideas',
        text: 'Pick up to three concepts worth understanding, with no forced filler.',
        status: 'Preview',
      },
    ],
  },
  {
    title: 'Make it yours.',
    text: 'A little practice today. Something you remember tomorrow.',
    cards: [
      {
        icon: '?',
        title: 'Check your understanding',
        text: 'Try a short question and keep your answer and feedback.',
        status: 'Preview',
      },
      {
        icon: '↗',
        title: 'Try a small challenge',
        text: 'Put an idea into practice with a focused task from your project.',
        status: 'Planned',
      },
      {
        icon: '↺',
        title: 'Build lasting knowledge',
        text: 'Keep an honest learning history and revisit ideas when they are due.',
        status: 'Planned',
      },
    ],
  },
  {
    title: 'Your tools. Your choice.',
    text: 'One place to understand AI-assisted work, across the tools you use.',
    cards: [
      {
        icon: '>_',
        title: 'Codex CLI',
        text: 'Local capture verified with version 0.151.0.',
        status: 'Available',
      },
      {
        icon: '▣',
        title: 'Codex Desktop',
        text: 'Planned session capture from the Codex desktop app.',
        status: 'Planned',
      },
      {
        icon: '⌥',
        title: 'Claude Code',
        text: 'A planned connection for your Claude Code sessions.',
        status: 'Planned',
      },
      {
        icon: '◈',
        title: 'Claude Cowork',
        text: 'Planned support for your work in Cowork.',
        status: 'Planned',
      },
    ],
  },
] as const;

export function Home({
  native,
  dark = false,
  visible = true,
  onStarted,
  onConnect,
  onSettings,
  workspace,
  onNavigate,
}: {
  native: boolean;
  dark?: boolean;
  visible?: boolean;
  onStarted?: () => void;
  onConnect: () => void;
  onSessions: () => void;
  onSettings: () => void;
  workspace?: LocalWorkspace;
  onNavigate?: (request: Omit<SessionNavigation, 'token'>) => void;
}) {
  const [stage, setStage] = useState<'welcome' | 'tour' | 'guide'>('welcome');
  const [slide, setSlide] = useState(0);
  const heading = useRef<HTMLHeadingElement>(null);
  const active = useWindowActive();
  const current = slides[slide] ?? slides[0];
  useEffect(() => {
    if (visible && stage !== 'welcome') heading.current?.focus();
  }, [stage, slide, visible]);
  const homeChecked = useRef(false);
  useEffect(() => {
    if (!workspace?.preferencesReady || homeChecked.current) return;
    if (!workspace.preferences.homeReached) {
      homeChecked.current = true;
      return;
    }
    const timer = setTimeout(() => {
      homeChecked.current = true;
      setStage('guide');
      onStarted?.();
    }, 0);
    return () => clearTimeout(timer);
  }, [
    workspace?.preferencesReady,
    workspace?.preferences.homeReached,
    onStarted,
  ]);
  function enterHome() {
    setStage('guide');
    workspace?.reachHome();
  }
  if (
    native &&
    workspace &&
    (!workspace.preferencesReady ||
      (stage === 'welcome' && workspace.preferences.homeReached))
  ) {
    return (
      <section className="bento-loading">
        <p role="status">Loading your Home…</p>
        {workspace.error && (
          <div role="alert">
            <p>{workspace.error}</p>
            <button onClick={() => void workspace.refresh()}>
              Retry local data
            </button>
            <button onClick={onSettings}>Open settings</button>
          </div>
        )}
      </section>
    );
  }
  function start() {
    setStage('tour');
    onStarted?.();
  }
  if (stage === 'welcome')
    return (
      <section className="welcome-entry" aria-labelledby="welcome-title">
        <img className="welcome-entry-character" src={character} alt="" />
        <p className="welcome-kicker">A little companion for your big ideas</p>
        <h2 id="welcome-title">
          <BlurText text="Hello, I’m mochi." active={visible && active} />
        </h2>
        <p className="welcome-entry-text">
          Build with AI. Understand what you built.
        </p>
        <SpecularButton dark={dark} active={visible} onClick={start}>
          Get Started <span aria-hidden="true">↗</span>
        </SpecularButton>
        <span className="welcome-entry-note">Your work. Your pace.</span>
      </section>
    );
  if (stage === 'tour')
    return (
      <section className="intro-tour" aria-label="About mochi">
        <div className="intro-topline">
          <span className="welcome-kicker">
            Meet mochi · {slide + 1} / {slides.length}
          </span>
          <button className="quiet-button" onClick={enterHome}>
            Skip tour
          </button>
        </div>
        <div className="intro-heading" aria-live="polite" aria-atomic="true">
          <h2 ref={heading} tabIndex={-1}>
            {current.title}
          </h2>
          <p>{current.text}</p>
        </div>
        <div
          className={`intro-cards${current.cards.length === 4 ? ' intro-cards-four' : ''}`}
        >
          {current.cards.map((card) => (
            <SpotlightCard key={card.title}>
              <div className="intro-card-top">
                <span className="intro-card-icon" aria-hidden="true">
                  {card.icon}
                </span>
                <span
                  className={`feature-status status-${card.status.toLowerCase()}`}
                >
                  {card.status}
                </span>
              </div>
              <h3>{card.title}</h3>
              <p>{card.text}</p>
            </SpotlightCard>
          ))}
        </div>
        <p className="intro-availability">
          Preview features still need a real AI trial. Planned features aren’t
          available yet.
        </p>
        <div className="intro-footer">
          <button
            disabled={slide === 0}
            onClick={() => setSlide((value) => Math.max(0, value - 1))}
          >
            Back
          </button>
          <div className="slide-controls" aria-label="Introduction slides">
            {slides.map((item, index) => (
              <button
                key={item.title}
                aria-label={`Slide ${index + 1}: ${item.title}`}
                aria-pressed={slide === index}
                onClick={() => setSlide(index)}
              >
                {index + 1}
              </button>
            ))}
          </div>
          <button
            className="intro-next"
            onClick={() =>
              slide === slides.length - 1
                ? enterHome()
                : setSlide((value) => value + 1)
            }
          >
            {slide === slides.length - 1 ? 'Let’s begin' : 'Next'}{' '}
            <span aria-hidden="true">→</span>
          </button>
        </div>
      </section>
    );
  return (
    <HomeBento
      native={native}
      workspace={workspace}
      headingRef={heading}
      onNavigate={onNavigate}
      onLegacyConnect={onConnect}
      onSettings={onSettings}
      onReplay={() => {
        setSlide(0);
        setStage('tour');
      }}
    />
  );
}
