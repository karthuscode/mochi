import { act, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { AuraBackground } from './AuraBackground';

vi.mock('./LiquidEther', () => ({
  LiquidEther: ({ active }: { active: boolean }) => (
    <div data-testid="fluid" data-active={String(active)} />
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
    expect(screen.getByTestId('fluid')).toHaveAttribute('data-active', 'true');
    focus.mockReturnValue(false);
    fireEvent(window, new Event('blur'));
    expect(screen.getByTestId('fluid')).toHaveAttribute('data-active', 'false');
    focus.mockReturnValue(true);
    visibility.mockReturnValue('hidden');
    fireEvent(document, new Event('visibilitychange'));
    expect(screen.getByTestId('fluid')).toHaveAttribute('data-active', 'false');
    visibility.mockReturnValue('visible');
    fireEvent(document, new Event('visibilitychange'));
    expect(screen.getByTestId('fluid')).toHaveAttribute('data-active', 'true');
  });
  it('removes the animated canvas when reduced motion becomes enabled', () => {
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
    expect(screen.getByTestId('fluid')).toBeInTheDocument();
    act(() => {
      reduced = true;
      [...listeners].forEach((notify) => notify());
    });
    expect(screen.queryByTestId('fluid')).not.toBeInTheDocument();
    view.unmount();
    expect(listeners.size).toBe(0);
  });
});
