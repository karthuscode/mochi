import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { LocalCapture } from './LocalCapture';
import { localCapture } from './native/local-capture';
import type { ProjectView, SessionList } from './native/local-capture';

vi.mock('./native/local-capture', () => ({
  localCapture: {
    projects: vi.fn(),
    status: vi.fn(),
    sessions: vi.fn(),
    preview: vi.fn(),
    apply: vi.fn(),
    tracking: vi.fn(),
    deleteProject: vi.fn(),
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
  vi.mocked(localCapture.sessions).mockResolvedValue(page);
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
});
async function selectFirst() {
  await screen.findByRole('option', { name: first.name });
  fireEvent.change(screen.getByLabelText('Project', { exact: true }), {
    target: { value: first.id },
  });
  await screen.findByText('interrupted · partial coverage');
}
describe('local capture interface boundaries', () => {
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
