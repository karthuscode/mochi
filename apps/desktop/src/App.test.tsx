import { fireEvent, render, screen } from '@testing-library/react';
import { mockIPC } from '@tauri-apps/api/mocks';
import { describe, expect, it, vi } from 'vitest';
import { App } from './App';

describe('foundation screen', () => {
  it('renders the shell through the shared UI package', () => {
    render(<App />);
    expect(screen.getByRole('main')).toContainElement(
      screen.getByRole('heading', { name: 'Mochi', level: 1 }),
    );
    expect(screen.getByText('V1 development build')).toBeVisible();
  });

  it('validates and displays a native IPC response', async () => {
    vi.stubGlobal('isTauri', true);
    mockIPC((command) => {
      expect(command).toBe('get_app_info');
      return { schemaVersion: 1, name: 'Mochi', version: '0.1.0' };
    });
    render(<App />);
    fireEvent.click(
      screen.getByRole('button', { name: 'Check desktop connection' }),
    );
    expect(
      await screen.findByText('Desktop connected · Mochi 0.1.0'),
    ).toBeVisible();
  });

  it.each(['reject', 'malformed'])(
    'shows a safe error for %s IPC',
    async (failure) => {
      vi.stubGlobal('isTauri', true);
      mockIPC(() => {
        if (failure === 'reject') throw new Error('Internal native failure');
        return { schemaVersion: 2 };
      });
      render(<App />);
      fireEvent.click(
        screen.getByRole('button', { name: 'Check desktop connection' }),
      );
      expect(
        await screen.findByText(
          'Desktop connection failed. Restart Mochi and try again.',
        ),
      ).toBeVisible();
      expect(
        screen.queryByText('Internal native failure'),
      ).not.toBeInTheDocument();
    },
  );
});
