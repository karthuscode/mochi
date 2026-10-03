import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { useState, type ComponentProps } from 'react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { HomeBento } from './HomeBento';
import { homeNative } from './native/home';
import type { LocalWorkspace, ProjectFolderDraft } from './useLocalWorkspace';
import type { Lesson } from './native/learning';
vi.mock('./native/home', () => ({ homeNative: { pickFolder: vi.fn() } }));
const project = {
  id: '10000000-0000-4000-8000-000000000001',
  name: 'Synthetic project',
  root: '/synthetic/project',
  tracking: true,
  policyRevision: 1,
};
const session = {
  id: '10000000-0000-4000-8000-000000000002',
  startedAt: '2026-10-03T12:00:00Z',
  endedAt: '2026-10-03T12:01:00Z',
  captureState: 'finalized',
  coverage: 'partial',
  revision: 2,
  paused: false,
  restarted: false,
  lateEvidence: false,
  continuationOf: null,
};
function workspace(overrides: Partial<LocalWorkspace> = {}): LocalWorkspace {
  return {
    projects: [],
    projectId: '',
    folderDraft: null,
    setFolderDraft: vi.fn(),
    sessions: { items: [], next: null },
    latest: null,
    sessionsReady: true,
    loaded: true,
    error: '',
    status: null,
    analysis: {
      enabled: false,
      keyConfigured: false,
      running: false,
      message: '',
      model: 'synthetic',
    },
    lesson: null,
    learningError: '',
    preferences: { schemaVersion: 1, homeReached: true, projectId: null },
    preferencesReady: true,
    preferenceNotice: '',
    selectProject: vi.fn(),
    setSessions: vi.fn(),
    refresh: vi.fn(),
    setBrowsingOlder: vi.fn(),
    isSelectedProject: vi.fn(),
    selectionToken: vi.fn(),
    isCurrentSelection: vi.fn(),
    reachHome: vi.fn(),
    ...overrides,
  };
}
function props() {
  return {
    native: true,
    onNavigate: vi.fn(),
    onSettings: vi.fn(),
    onReplay: vi.fn(),
    onLegacyConnect: vi.fn(),
  };
}
function TestBento(p: ComponentProps<typeof HomeBento>) {
  const [folderDraft, setFolderDraft] = useState<ProjectFolderDraft | null>(
    null,
  );
  return (
    <HomeBento
      {...p}
      workspace={
        p.workspace
          ? { ...p.workspace, folderDraft, setFolderDraft }
          : undefined
      }
    />
  );
}
beforeEach(() => {
  vi.clearAllMocks();
  vi.mocked(homeNative.pickFolder).mockResolvedValue('/synthetic/example');
});
describe('Bento project entry', () => {
  it('selects an already approved matching folder without another approval draft', async () => {
    vi.mocked(homeNative.pickFolder).mockResolvedValue(project.root);
    const state = workspace({ projects: [project] });
    render(<TestBento {...props()} workspace={state} />);
    fireEvent.click(
      screen.getAllByRole('button', { name: 'Choose a project folder' })[0]!,
    );
    await waitFor(() =>
      expect(state.selectProject).toHaveBeenCalledWith(project.id),
    );
    expect(screen.queryByLabelText('Project name')).not.toBeInTheDocument();
  });
  it('selects a folder then a tool and forwards only a memory draft, without approval', async () => {
    const p = props();
    render(<TestBento {...p} workspace={workspace()} />);
    fireEvent.click(
      screen.getAllByRole('button', { name: /Choose a project folder/ })[0]!,
    );
    expect(await screen.findByText('/synthetic/example')).toBeVisible();
    expect(screen.getByLabelText('Project name')).toHaveValue('example');
    expect(screen.getByRole('radio', { name: /Codex CLI/ })).not.toBeChecked();
    fireEvent.change(screen.getByLabelText('Project name'), {
      target: { value: 'Safe alias' },
    });
    fireEvent.click(screen.getByRole('radio', { name: /Codex CLI/ }));
    fireEvent.click(
      screen.getByRole('button', { name: /Review project access/ }),
    );
    expect(p.onNavigate).toHaveBeenCalledWith({
      target: 'approval',
      draft: { path: '/synthetic/example', name: 'Safe alias' },
    });
  });
  it.each(['Codex Desktop', 'Claude Code', 'Claude Cowork'])(
    'keeps %s unavailable with an escape',
    async (name) => {
      const p = props();
      render(<TestBento {...p} workspace={workspace()} />);
      fireEvent.click(
        screen.getAllByRole('button', { name: /Choose a project folder/ })[0]!,
      );
      await screen.findByText('/synthetic/example');
      fireEvent.click(screen.getByRole('radio', { name: new RegExp(name) }));
      expect(
        screen.getByText('This connection is not available yet.'),
      ).toBeVisible();
      expect(
        screen.queryByRole('button', { name: /Review project access/ }),
      ).not.toBeInTheDocument();
      expect(p.onNavigate).not.toHaveBeenCalled();
      fireEvent.click(
        screen.getByRole('button', { name: /Choose another tool/ }),
      );
      expect(screen.getByRole('radio', { name: /Codex CLI/ })).toHaveFocus();
      fireEvent.click(screen.getByRole('button', { name: 'Back to projects' }));
      expect(screen.queryByText('/synthetic/example')).not.toBeInTheDocument();
    },
  );
  it('cancels without changing the selected project and gives a safe picker failure fallback', async () => {
    vi.mocked(homeNative.pickFolder)
      .mockResolvedValueOnce(null)
      .mockRejectedValueOnce(new Error('private diagnostic'));
    const p = props();
    render(
      <TestBento
        {...p}
        workspace={workspace({ projects: [project], projectId: project.id })}
      />,
    );
    fireEvent.click(
      screen.getByRole('button', { name: 'Choose a project folder' }),
    );
    await screen.findByRole('button', { name: 'Choose a project folder' });
    expect(screen.getByLabelText('Learning project')).toHaveValue(project.id);
    fireEvent.click(
      screen.getByRole('button', { name: 'Choose a project folder' }),
    );
    expect(await screen.findByRole('alert')).not.toHaveTextContent(
      'private diagnostic',
    );
    fireEvent.click(
      screen.getByRole('button', { name: 'Enter folder in Sessions' }),
    );
    expect(p.onLegacyConnect).toHaveBeenCalledOnce();
  });
  it('does not call native code in browser preview', () => {
    render(<TestBento {...props()} native={false} workspace={workspace()} />);
    screen
      .getAllByRole('button', { name: /Choose a project folder/ })
      .forEach((b) => {
        expect(b).toBeDisabled();
        fireEvent.click(b);
      });
    expect(homeNative.pickFolder).not.toHaveBeenCalled();
  });
  it('keeps storage failures distinct from an empty project list', () => {
    render(
      <TestBento
        {...props()}
        workspace={workspace({ error: 'Local storage is unavailable.' })}
      />,
    );
    expect(screen.getByRole('alert')).toHaveTextContent(
      'Local storage is unavailable.',
    );
    screen
      .getAllByRole('button', { name: /Choose a project folder/ })
      .forEach((b) => expect(b).toBeDisabled());
  });
});
describe('Truthful next steps', () => {
  it.each([true, false])(
    'shows analysis progress or insufficient context without claiming an explanation (running=%s)',
    (running) => {
      render(
        <TestBento
          {...props()}
          workspace={workspace({
            projects: [project],
            projectId: project.id,
            latest: session,
            lesson: {
              sessionId: session.id,
              status: 'insufficient_context',
              stale: false,
              inputRevision: session.revision,
            } as Lesson,
            analysis: {
              enabled: true,
              keyConfigured: true,
              running,
              message: '',
              model: 'synthetic',
            },
          })}
        />,
      );
      expect(
        screen.getByText(
          running ? 'Your analysis is in progress.' : 'Review missing context.',
        ),
      ).toBeVisible();
      expect(
        screen.queryByRole('button', { name: /Open explanation/ }),
      ).not.toBeInTheDocument();
    },
  );
  it('does not claim a connection is verified before the first session', () => {
    render(
      <TestBento
        {...props()}
        workspace={workspace({ projects: [project], projectId: project.id })}
      />,
    );
    expect(screen.getByText('Make your first small change.')).toBeVisible();
    expect(screen.getByText(/waiting for its first session/)).toBeVisible();
  });
  it('shows partial capture and routes a real session', () => {
    const p = props();
    render(
      <TestBento
        {...p}
        workspace={workspace({
          projects: [project],
          projectId: project.id,
          latest: session,
        })}
      />,
    );
    expect(screen.getByText('Some activity may be missing.')).toBeVisible();
    expect(screen.getByText('Set up optional explanations.')).toBeVisible();
    fireEvent.click(screen.getByRole('button', { name: 'View session →' }));
    expect(p.onNavigate).toHaveBeenCalledWith({
      target: 'session',
      projectId: project.id,
      sessionId: session.id,
    });
  });
  it('requires current independent remote permission after key setup', () => {
    render(
      <TestBento
        {...props()}
        workspace={workspace({
          projects: [project],
          projectId: project.id,
          latest: session,
          analysis: {
            enabled: false,
            keyConfigured: true,
            running: false,
            message: '',
            model: 'synthetic',
          },
        })}
      />,
    );
    expect(screen.getByText('Choose whether to analyze.')).toBeVisible();
    expect(
      screen.getByText(/exact session context before any send/),
    ).toBeVisible();
  });
  it.each([true, false])(
    'opens only a current valid explanation (stale=%s)',
    (stale) => {
      const lesson = {
        sessionId: session.id,
        stale,
        inputRevision: session.revision,
        status: 'internal_explanation',
      } as Lesson;
      render(
        <TestBento
          {...props()}
          workspace={workspace({
            projects: [project],
            projectId: project.id,
            latest: session,
            lesson,
            analysis: {
              enabled: true,
              keyConfigured: true,
              running: false,
              message: '',
              model: 'synthetic',
            },
          })}
        />,
      );
      if (stale)
        expect(
          screen.queryByRole('button', { name: /Open explanation/ }),
        ).not.toBeInTheDocument();
      else
        expect(
          screen.getByRole('button', { name: /Open explanation & self-check/ }),
        ).toBeVisible();
    },
  );
});
