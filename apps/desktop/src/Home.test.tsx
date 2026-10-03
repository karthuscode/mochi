import { fireEvent, render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import { Home } from './Home';

function callbacks() {
  return {
    onStarted: vi.fn(),
    onConnect: vi.fn(),
    onSessions: vi.fn(),
    onSettings: vi.fn(),
  };
}
function start() {
  fireEvent.click(screen.getByRole('button', { name: /Get Started/ }));
}

describe('Welcome and guided introduction', () => {
  it('starts with a greeting and a single deliberate start action', () => {
    const actions = callbacks();
    render(<Home native={false} {...actions} />);
    expect(
      screen.getByRole('heading', { name: 'Hello, I’m mochi.' }),
    ).toBeVisible();
    expect(
      screen.queryByRole('button', { name: /Connect a project/ }),
    ).not.toBeInTheDocument();
    expect(
      screen.queryByRole('heading', { name: 'Your first session' }),
    ).not.toBeInTheDocument();
    expect(actions.onStarted).not.toHaveBeenCalled();
    start();
    expect(actions.onStarted).toHaveBeenCalledOnce();
    expect(
      screen.getByRole('heading', { name: 'Your work, remembered.' }),
    ).toHaveFocus();
    expect(actions.onConnect).not.toHaveBeenCalled();
    expect(actions.onSessions).not.toHaveBeenCalled();
    expect(actions.onSettings).not.toHaveBeenCalled();
  });

  it('manually presents existing and planned features, with bounded navigation', () => {
    const actions = callbacks();
    render(<Home native={false} {...actions} />);
    start();
    expect(screen.getByRole('button', { name: 'Back' })).toBeDisabled();
    fireEvent.click(screen.getByRole('button', { name: /Next/ }));
    expect(
      screen.getByRole('heading', { name: 'Understand the why.' }),
    ).toHaveFocus();
    expect(screen.getByText('Planned')).toBeVisible();
    expect(screen.getAllByText('Preview')).toHaveLength(2);
    fireEvent.click(screen.getByRole('button', { name: /Slide 3/ }));
    expect(
      screen.getByRole('heading', { name: 'Build lasting knowledge' }),
    ).toBeVisible();
    expect(screen.getAllByText('Planned')).toHaveLength(2);
    fireEvent.click(screen.getByRole('button', { name: /Slide 4/ }));
    expect(screen.getAllByText('Planned')).toHaveLength(3);
    expect(screen.getByText(/0.151.0/)).toBeVisible();
    expect(
      screen.queryByRole('button', { name: /Connect/ }),
    ).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: /Let’s begin/ }));
    expect(
      screen.getByRole('heading', { name: 'Make more of what you build.' }),
    ).toHaveFocus();
    expect(
      screen.getByText(/Project selection requires the macOS app/),
    ).toBeVisible();
    expect(actions.onConnect).not.toHaveBeenCalled();
    expect(actions.onSessions).not.toHaveBeenCalled();
    expect(actions.onSettings).not.toHaveBeenCalled();
  });

  it('allows skipping/replaying and routes only deliberate guide actions', () => {
    const actions = callbacks();
    render(<Home native {...actions} />);
    start();
    fireEvent.click(screen.getByRole('button', { name: 'Skip tour' }));
    fireEvent.click(
      screen.getByRole('button', { name: 'Show first-session guide' }),
    );
    expect(
      screen.getByText('Choose and approve one project folder.'),
    ).toBeVisible();
    fireEvent.click(
      screen.getByRole('button', { name: /Open learning settings/ }),
    );
    expect(actions.onSettings).toHaveBeenCalledOnce();
    expect(actions.onConnect).not.toHaveBeenCalled();
    expect(actions.onSessions).not.toHaveBeenCalled();
    fireEvent.click(
      screen.getByRole('button', { name: 'Replay introduction' }),
    );
    expect(screen.getByRole('button', { name: /Slide 1/ })).toHaveAttribute(
      'aria-pressed',
      'true',
    );
    expect(actions.onStarted).toHaveBeenCalledOnce();
  });
});
