import {
  act,
  fireEvent,
  render,
  screen,
  waitFor,
} from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { LocalCapture } from './LocalCapture';
import { localCapture } from './native/local-capture';
import type { ProjectView, SessionList } from './native/local-capture';
vi.mock('./Learning', () => ({
  LearningPanel: () => <p>Internal explanation preview</p>,
}));

vi.mock('./native/local-capture', () => ({
  localCapture: {
    projects: vi.fn(),
    status: vi.fn(),
    sessions: vi.fn(),
    preview: vi.fn(),
    apply: vi.fn(),
    tracking: vi.fn(),
    deleteProject: vi.fn(),
    detail: vi.fn(),
  },
}));
const first: ProjectView = {
  id: '10000000-0000-4000-8000-000000000001',
  name: 'Synthetic project',
  root: '/synthetic/project',
  tracking: false,
  policyRevision: 1,
};
const second: ProjectView = {
  ...first,
  id: '10000000-0000-4000-8000-000000000002',
  name: 'Other synthetic project',
};
const page: SessionList = {
  items: [
    {
      id: '10000000-0000-4000-8000-000000000003',
      startedAt: '2026-10-02T12:00:00Z',
      endedAt: null,
      captureState: 'interrupted',
      coverage: 'partial',
      revision: 1,
      paused: false,
      restarted: true,
      lateEvidence: false,
      continuationOf: null,
    },
  ],
  next: null,
};
beforeEach(() => {
  vi.clearAllMocks();
  vi.mocked(localCapture.projects).mockResolvedValue([first, second]);
  vi.mocked(localCapture.status).mockResolvedValue({
    schemaVersion: 1,
    message: 'Local capture ready.',
    spoolEvictionCount: 0,
    remoteAnalysisEnabled: false,
  });
  vi.mocked(localCapture.sessions).mockReset().mockResolvedValue(page);
  vi.mocked(localCapture.preview).mockResolvedValue({
    schemaVersion: 1,
    planId: first.id,
    action: 'install',
    target: '/synthetic/project/.codex/hooks.json',
    ownedCommand: 'synthetic-helper',
    events: ['Stop'],
    changesConfiguration: true,
    requiresCodexTrustReview: true,
    backupRetentionDays: 7,
  });
  vi.mocked(localCapture.apply).mockResolvedValue();
  vi.mocked(localCapture.deleteProject).mockResolvedValue();
  vi.mocked(localCapture.detail).mockResolvedValue({
    session: page.items[0]!,
    events: [],
    eventCount: 0,
    nextSequence: null,
    code: [],
    gitNotice: 'No code fixture',
  });
});
async function selectFirst() {
  await screen.findByRole('option', { name: first.name });
  fireEvent.change(screen.getByLabelText('Project', { exact: true }), {
    target: { value: first.id },
  });
  await screen.findByText('interrupted · partial coverage');
}
describe('local capture interface boundaries', () => {
  it('opens and focuses a targeted session after detail rendering, including a repeated request', async () => {
    const { rerender } = render(<LocalCapture />);
    await selectFirst();
    const navigation = {
      token: 1,
      projectId: first.id,
      sessionId: page.items[0]!.id,
      target: 'session' as const,
    };
    rerender(<LocalCapture navigation={navigation} />);
    await waitFor(() =>
      expect(
        screen.getByRole('heading', { name: 'Observed session' }),
      ).toHaveFocus(),
    );
    screen.getByLabelText('Project', { exact: true }).focus();
    rerender(<LocalCapture navigation={{ ...navigation, token: 2 }} />);
    await waitFor(() =>
      expect(
        screen.getByRole('heading', { name: 'Observed session' }),
      ).toHaveFocus(),
    );
  });
  it('cannot publish an old session page after switching away and back', async () => {
    render(<LocalCapture />);
    await selectFirst();
    let finish!: (value: SessionList) => void;
    vi.mocked(localCapture.sessions).mockReturnValueOnce(
      new Promise((resolve) => {
        finish = resolve;
      }),
    );
    fireEvent.click(screen.getByRole('button', { name: 'Latest sessions' }));
    fireEvent.change(screen.getByLabelText('Project', { exact: true }), {
      target: { value: second.id },
    });
    fireEvent.change(screen.getByLabelText('Project', { exact: true }), {
      target: { value: first.id },
    });
    await act(async () =>
      finish({
        items: [{ ...page.items[0]!, captureState: 'old unsafe page' }],
        next: null,
      }),
    );
    expect(screen.queryByText(/old unsafe page/)).not.toBeInTheDocument();
  });
  it('keeps an exact connection disabled until its independent local consent is checked', async () => {
    render(<LocalCapture />);
    await selectFirst();
    expect(localCapture.tracking).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole('button', { name: 'Review connection' }));
    const approve = await screen.findByRole('button', {
      name: 'Approve connection',
    });
    expect(approve).toBeDisabled();
    expect(localCapture.apply).not.toHaveBeenCalled();
    fireEvent.click(
      screen.getByRole('checkbox', {
        name: /Enable local capture and approve this exact/,
      }),
    );
    fireEvent.click(approve);
    await waitFor(() =>
      expect(localCapture.apply).toHaveBeenCalledWith(first.id, true),
    );
  });
  it('clears the old session page and preview immediately on a project switch', async () => {
    render(<LocalCapture />);
    await selectFirst();
    fireEvent.click(screen.getByRole('button', { name: 'Review connection' }));
    await screen.findByRole('button', { name: 'Approve connection' });
    vi.mocked(localCapture.sessions).mockReturnValueOnce(new Promise(() => {}));
    fireEvent.change(screen.getByLabelText('Project', { exact: true }), {
      target: { value: second.id },
    });
    expect(
      screen.queryByText('interrupted · partial coverage'),
    ).not.toBeInTheDocument();
    expect(
      screen.queryByRole('button', { name: 'Approve connection' }),
    ).not.toBeInTheDocument();
    expect(localCapture.apply).not.toHaveBeenCalled();
  });
  it('requires a separate deletion confirmation and allows keeping data', async () => {
    render(<LocalCapture />);
    await selectFirst();
    fireEvent.click(screen.getByText('Project details & disconnect'));
    fireEvent.click(
      screen.getByRole('button', { name: 'Delete project data' }),
    );
    expect(screen.getByRole('alertdialog')).toHaveTextContent(
      'Original project files and Codex history stay intact.',
    );
    expect(localCapture.deleteProject).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole('button', { name: 'Keep data' }));
    expect(screen.queryByRole('alertdialog')).not.toBeInTheDocument();
    expect(localCapture.deleteProject).not.toHaveBeenCalled();
  });
  it('shows a safe unavailable state without suggesting a new empty database', async () => {
    vi.mocked(localCapture.projects).mockRejectedValue(
      new Error('private internal path'),
    );
    render(<LocalCapture />);
    expect(await screen.findByRole('alert')).toHaveTextContent(
      'Local storage is unavailable.',
    );
    expect(screen.queryByText(/Your work\./)).not.toBeInTheDocument();
    expect(screen.queryByText('private internal path')).not.toBeInTheDocument();
  });
});
