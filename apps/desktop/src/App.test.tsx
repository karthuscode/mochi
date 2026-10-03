import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { mockIPC } from '@tauri-apps/api/mocks';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { App } from './App';
import { localCapture } from './native/local-capture';
import { homeNative, defaultHomePreferences } from './native/home';
import { learning } from './native/learning';

vi.mock('./background/DotField', () => ({
  DotField: ({ animated }: { animated: boolean }) => (
    <div data-testid="dot-field" data-animated={animated} />
  ),
}));

vi.mock('./native/local-capture', () => ({
  localCapture: {
    projects: vi.fn(),
    status: vi.fn(),
    pauseAll: vi.fn(),
    approve: vi.fn(),
    sessions: vi.fn(),
  },
}));
vi.mock('./native/home', () => ({
  defaultHomePreferences: {
    schemaVersion: 1,
    homeReached: false,
    projectId: null,
  },
  homeNative: { read: vi.fn(), save: vi.fn(), pickFolder: vi.fn() },
}));
vi.mock('./native/learning', () => ({
  learning: { status: vi.fn(), permission: vi.fn(), send: vi.fn() },
}));

beforeEach(() => {
  vi.clearAllMocks();
  vi.mocked(homeNative.read).mockResolvedValue(defaultHomePreferences);
  vi.mocked(homeNative.save).mockResolvedValue();
  vi.mocked(homeNative.pickFolder).mockResolvedValue('/synthetic/example');
  vi.mocked(localCapture.projects).mockResolvedValue([]);
  vi.mocked(localCapture.sessions).mockResolvedValue({ items: [], next: null });
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

async function enter() {
  const start = await screen.findByRole('button', { name: /Get Started/ });
  fireEvent.click(start);
}
async function guide() {
  await enter();
  const skip = screen.queryByRole('button', { name: 'Skip tour' });
  if (skip) fireEvent.click(skip);
}
async function settings() {
  if (!screen.queryByRole('button', { name: 'Settings' })) await enter();
  fireEvent.click(screen.getByRole('button', { name: 'Settings' }));
}

describe('personal MVP shell', () => {
  it('keeps a return route when startup storage fails and Settings is opened', async () => {
    vi.stubGlobal('isTauri', true);
    vi.mocked(localCapture.projects).mockRejectedValue(
      new Error('private storage diagnostic'),
    );
    render(<App />);
    expect(await screen.findByRole('alert')).toHaveTextContent(
      'Local storage is unavailable.',
    );
    fireEvent.click(screen.getByRole('button', { name: 'Open settings' }));
    expect(
      await screen.findByRole('heading', { name: 'Learning settings' }),
    ).toBeVisible();
    fireEvent.click(screen.getByRole('button', { name: 'Home' }));
    expect(
      screen.getByRole('button', { name: 'Retry local data' }),
    ).toBeVisible();
    expect(
      screen.queryByText('private storage diagnostic'),
    ).not.toBeInTheDocument();
  });
  it('prefills and focuses native folder approval, keeps it unchecked, then clears the completed Home draft', async () => {
    vi.stubGlobal('isTauri', true);
    const approved = {
      id: '10000000-0000-4000-8000-000000000001',
      name: 'Example',
      root: '/canonical/example',
      tracking: false,
      policyRevision: 1,
    };
    vi.mocked(localCapture.approve).mockImplementation(async () => {
      vi.mocked(localCapture.projects).mockResolvedValue([approved]);
      return approved;
    });
    render(<App />);
    await guide();
    fireEvent.click(
      screen.getAllByRole('button', { name: 'Choose a project folder' })[0]!,
    );
    await screen.findByText('/synthetic/example');
    fireEvent.click(screen.getByRole('radio', { name: /Codex CLI/ }));
    fireEvent.click(
      screen.getByRole('button', { name: 'Review project access' }),
    );
    await waitFor(() =>
      expect(screen.getByLabelText('Project alias')).toHaveFocus(),
    );
    expect(screen.getByLabelText('Project folder')).toHaveValue(
      '/synthetic/example',
    );
    expect(screen.getByLabelText('Project alias')).toHaveValue('example');
    const consent = screen.getByRole('checkbox', {
      name: /I approve this folder/,
    });
    expect(consent).not.toBeChecked();
    expect(
      screen.getByRole('button', { name: 'Approve folder' }),
    ).toBeDisabled();
    fireEvent.click(screen.getByRole('button', { name: 'Home' }));
    expect(screen.getByLabelText('Project name')).toHaveValue('example');
    fireEvent.click(screen.getByRole('button', { name: 'Sessions' }));
    fireEvent.click(consent);
    fireEvent.click(screen.getByRole('button', { name: 'Approve folder' }));
    await waitFor(() =>
      expect(screen.getByLabelText('Project')).toHaveValue(approved.id),
    );
    fireEvent.click(screen.getByRole('button', { name: 'Home' }));
    expect(await screen.findByText('Review your connection.')).toBeVisible();
    expect(screen.getByLabelText('Learning project')).toHaveValue(approved.id);
    expect(screen.queryByLabelText('Project name')).not.toBeInTheDocument();
    expect(learning.permission).not.toHaveBeenCalled();
    expect(learning.send).not.toHaveBeenCalled();
  });
  it('uses the selected mark with lowercase branding and labelled browser isolation', async () => {
    render(<App />);
    expect(screen.getByRole('main')).toContainElement(
      screen.getByRole('heading', { name: 'Welcome', level: 1 }),
    );
    expect(screen.getByText('mochi')).toBeVisible();
    expect(screen.getByText(/Appearance preview only/)).toBeVisible();
    await guide();
    fireEvent.click(screen.getByRole('button', { name: 'Sessions' }));
    fireEvent.click(screen.getByText('Approve a project folder'));
    expect(
      screen.getByRole('button', { name: 'Approve folder' }),
    ).toBeDisabled();
    expect(
      screen.queryByRole('button', { name: 'Pause all capture' }),
    ).not.toBeInTheDocument();
    await settings();
    fireEvent.click(screen.getByText('About & diagnostics'));
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
    await guide();
    fireEvent.click(screen.getByRole('button', { name: 'Sessions' }));
    fireEvent.click(screen.getByText('Approve a project folder'));
    fireEvent.change(screen.getByLabelText('Project alias'), {
      target: { value: 'Synthetic project' },
    });
    fireEvent.click(screen.getByRole('button', { name: 'Home' }));
    fireEvent.click(
      screen.getByRole('button', { name: 'Replay introduction' }),
    );
    fireEvent.click(screen.getByRole('button', { name: /Slide 2/ }));
    await settings();
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
    fireEvent.click(screen.getByRole('button', { name: 'Home' }));
    expect(screen.getByRole('button', { name: /Slide 2/ })).toHaveAttribute(
      'aria-pressed',
      'true',
    );
  });

  it('changes only window appearance and retains the choice across navigation', async () => {
    render(<App />);
    await settings();
    fireEvent.click(screen.getByRole('radio', { name: 'Light' }));
    expect(screen.getByRole('main')).toHaveClass('appearance-light');
    fireEvent.click(screen.getByRole('button', { name: 'Sessions' }));
    await settings();
    expect(screen.getByRole('radio', { name: 'Light' })).toBeChecked();
    fireEvent.click(screen.getByRole('radio', { name: 'Dark' }));
    expect(screen.getByRole('main')).toHaveClass('appearance-dark');
    expect(learning.permission).not.toHaveBeenCalled();
    expect(learning.send).not.toHaveBeenCalled();
  });

  it('keeps the background switch across navigation without native calls', async () => {
    render(<App />);
    expect(screen.getByTestId('dot-field')).toHaveAttribute(
      'data-animated',
      'true',
    );
    await settings();
    const toggle = screen.getByRole('checkbox', {
      name: 'Animated background',
    });
    expect(toggle).toBeChecked();
    fireEvent.click(toggle);
    expect(screen.getByTestId('dot-field')).toHaveAttribute(
      'data-animated',
      'false',
    );
    fireEvent.click(screen.getByRole('button', { name: 'Sessions' }));
    await settings();
    expect(
      screen.getByRole('checkbox', { name: 'Animated background' }),
    ).not.toBeChecked();
    expect(learning.permission).not.toHaveBeenCalled();
    expect(learning.send).not.toHaveBeenCalled();
  });

  it('honors reduced motion even with the animation preference enabled', async () => {
    vi.stubGlobal(
      'matchMedia',
      vi.fn((query: string) => ({
        matches: query === '(prefers-reduced-motion: reduce)',
        media: query,
        addEventListener: vi.fn(),
        removeEventListener: vi.fn(),
      })),
    );
    render(<App />);
    await settings();
    expect(screen.getByTestId('dot-field')).toHaveAttribute(
      'data-animated',
      'false',
    );
    expect(
      screen.getByRole('checkbox', { name: 'Animated background' }),
    ).toBeDisabled();
    expect(screen.getByText(/Reduce Motion setting/)).toBeVisible();
  });

  it('resolves system dark appearance and allows a light override', async () => {
    vi.stubGlobal(
      'matchMedia',
      vi.fn((query: string) => ({
        matches: query === '(prefers-color-scheme: dark)',
        media: query,
        addEventListener: vi.fn(),
        removeEventListener: vi.fn(),
      })),
    );
    render(<App />);
    expect(screen.getByRole('main')).toHaveClass('theme-dark');
    await settings();
    fireEvent.click(screen.getByRole('radio', { name: 'Light' }));
    expect(screen.getByRole('main')).toHaveClass('theme-light');
  });

  it('validates and displays a native IPC response in settings', async () => {
    vi.stubGlobal('isTauri', true);
    mockIPC((command) => {
      expect(command).toBe('get_app_info');
      return { schemaVersion: 1, name: 'Mochi', version: '0.1.0' };
    });
    render(<App />);
    await settings();
    fireEvent.click(screen.getByText('About & diagnostics'));
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
      await settings();
      fireEvent.click(screen.getByText('About & diagnostics'));
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
