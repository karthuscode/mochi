import { act, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { AuraBackground } from './AuraBackground';

vi.mock('./DotField', () => ({
  DotField: ({ active, animated }: { active: boolean; animated: boolean }) => (
    <div
      data-testid="dots"
      data-active={String(active)}
      data-animated={String(animated)}
    />
  ),
}));
afterEach(() => vi.restoreAllMocks());
describe('background accessibility and window activity', () => {
  it('pauses when focus or visibility is lost and resumes only in a visible focused window', () => {
    const focus = vi.spyOn(document, 'hasFocus').mockReturnValue(true);
    const visibility = vi
      .spyOn(document, 'visibilityState', 'get')
      .mockReturnValue('visible');
    render(<AuraBackground dark animated />);
    expect(screen.getByTestId('dots')).toHaveAttribute('data-active', 'true');
    focus.mockReturnValue(false);
    fireEvent(window, new Event('blur'));
    expect(screen.getByTestId('dots')).toHaveAttribute('data-active', 'false');
    focus.mockReturnValue(true);
    visibility.mockReturnValue('hidden');
    fireEvent(document, new Event('visibilitychange'));
    expect(screen.getByTestId('dots')).toHaveAttribute('data-active', 'false');
    visibility.mockReturnValue('visible');
    fireEvent(document, new Event('visibilitychange'));
    expect(screen.getByTestId('dots')).toHaveAttribute('data-active', 'true');
  });
  it('keeps static dots when reduced motion becomes enabled', () => {
    let reduced = false;
    const listeners = new Set<() => void>();
    vi.stubGlobal('matchMedia', (query: string) => ({
      matches: query === '(prefers-reduced-motion: reduce)' && reduced,
      addEventListener: (_: string, notify: () => void) =>
        listeners.add(notify),
      removeEventListener: (_: string, notify: () => void) =>
        listeners.delete(notify),
    }));
    const view = render(<AuraBackground dark animated />);
    expect(screen.getByTestId('dots')).toHaveAttribute('data-animated', 'true');
    act(() => {
      reduced = true;
      [...listeners].forEach((notify) => notify());
    });
    expect(screen.getByTestId('dots')).toHaveAttribute(
      'data-animated',
      'false',
    );
    view.unmount();
    expect(listeners.size).toBe(0);
  });
  it('removes all decoration in forced colors', () => {
    vi.stubGlobal('matchMedia', (query: string) => ({
      matches: query === '(forced-colors: active)',
      addEventListener: vi.fn(),
      removeEventListener: vi.fn(),
    }));
    render(<AuraBackground dark animated />);
    expect(screen.queryByTestId('dots')).not.toBeInTheDocument();
  });
});
