import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { mockIPC } from '@tauri-apps/api/mocks';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { App } from './App';
import { localCapture } from './native/local-capture';
import { learning } from './native/learning';

vi.mock('./native/local-capture', () => ({
  localCapture: {
    projects: vi.fn(),
    status: vi.fn(),
    pauseAll: vi.fn(),
    approve: vi.fn(),
  },
}));
vi.mock('./native/learning', () => ({
  learning: { status: vi.fn(), permission: vi.fn(), send: vi.fn() },
}));

beforeEach(() => {
  vi.clearAllMocks();
  vi.mocked(localCapture.projects).mockResolvedValue([]);
  vi.mocked(localCapture.status).mockResolvedValue({
    schemaVersion: 1,
    message: 'Local capture ready.',
    spoolEvictionCount: 0,
    remoteAnalysisEnabled: false,
  });
  vi.mocked(learning.status).mockResolvedValue({
    keyConfigured: false,
    enabled: false,
    running: false,
    model: 'gpt-4o-mini-2024-07-18',
    message: 'Remote analysis is off.',
  });
});

function settings() {
  fireEvent.click(screen.getByRole('button', { name: 'Settings' }));
}

describe('personal MVP shell', () => {
  it('uses the selected mark with lowercase branding and labelled browser isolation', async () => {
    render(<App />);
    expect(screen.getByRole('main')).toContainElement(
      screen.getByRole('heading', { name: 'Sessions', level: 1 }),
    );
    expect(screen.getByText('mochi')).toBeVisible();
    expect(screen.getByText(/Appearance preview only/)).toBeVisible();
    fireEvent.click(
      screen.getByRole('link', { name: /Connect your first project/ }),
    );
    expect(
      screen.getByRole('button', { name: 'Approve folder' }),
    ).toBeDisabled();
    expect(
      screen.getByRole('button', { name: 'Pause all capture' }),
    ).toBeDisabled();
    settings();
    expect(
      screen.getByRole('button', { name: 'Check desktop connection' }),
    ).toBeDisabled();
    await new Promise((resolve) => setTimeout(resolve, 5));
    expect(localCapture.projects).not.toHaveBeenCalled();
    expect(learning.status).not.toHaveBeenCalled();
  });

  it('keeps project drafts across navigation without granting either permission', async () => {
    vi.stubGlobal('isTauri', true);
    render(<App />);
    await waitFor(() => expect(localCapture.projects).toHaveBeenCalled());
    fireEvent.click(
      screen.getByRole('link', { name: /Connect your first project/ }),
    );
    fireEvent.change(screen.getByLabelText('Project alias'), {
      target: { value: 'Synthetic project' },
    });
    settings();
    expect(await screen.findByText('Remote analysis is off.')).toBeVisible();
    expect(
      screen.getByRole('checkbox', { name: /Allow remote analysis controls/ }),
    ).not.toBeChecked();
    fireEvent.click(screen.getByRole('button', { name: 'Sessions' }));
    expect(screen.getByLabelText('Project alias')).toHaveValue(
      'Synthetic project',
    );
    expect(
      screen.getByRole('button', { name: 'Approve folder' }),
    ).toBeDisabled();
    expect(localCapture.approve).not.toHaveBeenCalled();
    expect(learning.permission).not.toHaveBeenCalled();
    expect(learning.send).not.toHaveBeenCalled();
  });

  it('changes only window appearance and retains the choice across navigation', () => {
    render(<App />);
    settings();
    fireEvent.click(screen.getByRole('radio', { name: 'Light' }));
    expect(screen.getByRole('main')).toHaveClass('appearance-light');
    fireEvent.click(screen.getByRole('button', { name: 'Sessions' }));
    settings();
    expect(screen.getByRole('radio', { name: 'Light' })).toBeChecked();
    fireEvent.click(screen.getByRole('radio', { name: 'Dark' }));
    expect(screen.getByRole('main')).toHaveClass('appearance-dark');
    expect(learning.permission).not.toHaveBeenCalled();
    expect(learning.send).not.toHaveBeenCalled();
  });

  it('validates and displays a native IPC response in settings', async () => {
    vi.stubGlobal('isTauri', true);
    mockIPC((command) => {
      expect(command).toBe('get_app_info');
      return { schemaVersion: 1, name: 'Mochi', version: '0.1.0' };
    });
    render(<App />);
    settings();
    fireEvent.click(
      screen.getByRole('button', { name: 'Check desktop connection' }),
    );
    expect(
      await screen.findByText('Desktop connected · mochi 0.1.0'),
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
      settings();
      fireEvent.click(
        screen.getByRole('button', { name: 'Check desktop connection' }),
      );
      expect(
        await screen.findByText(
          'Desktop connection failed. Restart mochi and try again.',
        ),
      ).toBeVisible();
      expect(
        screen.queryByText('Internal native failure'),
      ).not.toBeInTheDocument();
    },
  );
});
