import { fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { BlurText } from './BlurText';
import { SpecularButton } from './SpecularButton';
import { SpotlightCard } from './SpotlightCard';

const effects = vi.hoisted(() => ({ create: vi.fn(), dispose: vi.fn() }));
vi.mock('./specular-effect', () => ({ createSpecularEffect: effects.create }));
afterEach(() => vi.restoreAllMocks());

describe('introductory effects', () => {
  it('shows the whole greeting immediately with reduced motion', () => {
    vi.stubGlobal(
      'matchMedia',
      vi.fn((query: string) => ({
        matches: query === '(prefers-reduced-motion: reduce)',
        addEventListener: vi.fn(),
        removeEventListener: vi.fn(),
      })),
    );
    render(
      <h2>
        <BlurText text="Hello, I’m mochi." />
      </h2>,
    );
    expect(
      screen.getByRole('heading', { name: 'Hello, I’m mochi.' }),
    ).toBeVisible();
    for (const word of document.querySelectorAll('.blur-text-segment')) {
      expect(word).not.toHaveAttribute('style');
    }
  });

  it('keeps a single readable heading without intersection-observer support', () => {
    vi.stubGlobal('IntersectionObserver', undefined);
    render(
      <h2>
        <BlurText text="Hello, I’m mochi." active={false} />
      </h2>,
    );
    expect(
      screen.getAllByRole('heading', { name: 'Hello, I’m mochi.' }),
    ).toHaveLength(1);
    expect(document.querySelector('.blur-text-segments')).toHaveAttribute(
      'aria-hidden',
      'true',
    );
  });

  it('releases the button effect when the window loses focus or the page hides', () => {
    let focused = true;
    vi.spyOn(document, 'hasFocus').mockImplementation(() => focused);
    effects.create.mockReset().mockReturnValue(effects.dispose);
    effects.dispose.mockReset();
    const { rerender } = render(
      <SpecularButton dark active>
        Get Started
      </SpecularButton>,
    );
    expect(effects.create).toHaveBeenCalledOnce();
    focused = false;
    fireEvent(window, new Event('blur'));
    expect(effects.dispose).toHaveBeenCalledOnce();
    focused = true;
    fireEvent(window, new Event('focus'));
    expect(effects.create).toHaveBeenCalledTimes(2);
    rerender(
      <SpecularButton dark active={false}>
        Get Started
      </SpecularButton>,
    );
    expect(effects.dispose).toHaveBeenCalledTimes(2);
  });

  it('keeps native button semantics and click behavior without motion', () => {
    vi.spyOn(document, 'hasFocus').mockReturnValue(true);
    vi.stubGlobal(
      'matchMedia',
      vi.fn(() => ({
        matches: true,
        addEventListener: vi.fn(),
        removeEventListener: vi.fn(),
      })),
    );
    effects.create.mockClear();
    const click = vi.fn();
    render(
      <SpecularButton dark={false} onClick={click}>
        Get Started
      </SpecularButton>,
    );
    fireEvent.click(screen.getByRole('button', { name: 'Get Started' }));
    expect(click).toHaveBeenCalledOnce();
    expect(effects.create).not.toHaveBeenCalled();
  });

  it('keeps spotlight decoration separate from content and resets on leave', () => {
    render(
      <SpotlightCard>
        <h3>Planned review</h3>
        <p>Coming later</p>
      </SpotlightCard>,
    );
    const card = screen.getByRole('article');
    expect(
      screen.getByRole('heading', { name: 'Planned review' }),
    ).toBeVisible();
    fireEvent.pointerLeave(card);
    expect(card.style.getPropertyValue('--mouse-x')).toBe('50%');
    expect(card.style.getPropertyValue('--mouse-y')).toBe('50%');
    expect(card).not.toHaveAttribute('tabindex');
  });
});
