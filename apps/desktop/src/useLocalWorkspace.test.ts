import { act, renderHook, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { useLocalWorkspace } from './useLocalWorkspace';
import { localCapture } from './native/local-capture';
import type { SessionList } from './native/local-capture';
import { learning } from './native/learning';
import { defaultHomePreferences, homeNative } from './native/home';
vi.mock('./native/local-capture', () => ({
  localCapture: { projects: vi.fn(), status: vi.fn(), sessions: vi.fn() },
}));
vi.mock('./native/learning', () => ({
  learning: { status: vi.fn(), lesson: vi.fn() },
}));
vi.mock('./native/home', () => ({
  defaultHomePreferences: {
    schemaVersion: 1,
    homeReached: false,
    projectId: null,
  },
  homeNative: { read: vi.fn(), save: vi.fn() },
}));
const p1 = {
  id: '10000000-0000-4000-8000-000000000001',
  name: 'First',
  root: '/synthetic/first',
  tracking: false,
  policyRevision: 1,
};
const p2 = {
  ...p1,
  id: '10000000-0000-4000-8000-000000000002',
  name: 'Second',
};
const page: SessionList = {
  items: [
    {
      id: '10000000-0000-4000-8000-000000000003',
      startedAt: '2026-10-03T12:00:00Z',
      endedAt: null,
      captureState: 'idle',
      coverage: 'partial',
      revision: 1,
      paused: false,
      restarted: false,
      lateEvidence: false,
      continuationOf: null,
    },
  ],
  next: null,
};
beforeEach(() => {
  vi.resetAllMocks();
  vi.mocked(localCapture.projects).mockResolvedValue([p1, p2]);
  vi.mocked(localCapture.status).mockResolvedValue({
    schemaVersion: 1,
    message: 'Local status',
    remoteAnalysisEnabled: false,
    spoolEvictionCount: 0,
  });
  vi.mocked(localCapture.sessions).mockResolvedValue(page);
  vi.mocked(learning.status).mockResolvedValue({
    enabled: false,
    keyConfigured: false,
    running: false,
    model: 'synthetic',
    message: '',
  });
  vi.mocked(learning.lesson).mockResolvedValue(null);
  vi.mocked(homeNative.read).mockResolvedValue(defaultHomePreferences);
  vi.mocked(homeNative.save).mockResolvedValue();
});
describe('Shared Home/Sessions reads', () => {
  it('restores only a retained approved project and bypasses the introduction for existing users', async () => {
    vi.mocked(homeNative.read).mockResolvedValue({
      schemaVersion: 1,
      homeReached: false,
      projectId: p1.id,
    });
    const { result } = renderHook(() => useLocalWorkspace(true, false));
    await waitFor(() => expect(result.current.sessionsReady).toBe(true));
    expect(result.current.projectId).toBe(p1.id);
    expect(result.current.preferences.homeReached).toBe(true);
    expect(result.current.latest?.id).toBe(page.items[0]?.id);
    expect(homeNative.save).toHaveBeenCalledWith({
      schemaVersion: 1,
      homeReached: true,
      projectId: p1.id,
    });
  });
  it('returns to the picker when the remembered project was deleted', async () => {
    vi.mocked(homeNative.read).mockResolvedValue({
      schemaVersion: 1,
      homeReached: true,
      projectId: '10000000-0000-4000-8000-000000000099',
    });
    const { result } = renderHook(() => useLocalWorkspace(true, false));
    await waitFor(() => expect(result.current.preferencesReady).toBe(true));
    expect(result.current.projectId).toBe('');
    expect(result.current.preferences.homeReached).toBe(true);
  });
  it('ignores old session responses after a project switch, including an A→B→A switch', async () => {
    const { result } = renderHook(() => useLocalWorkspace(true, false, false));
    await waitFor(() => expect(result.current.loaded).toBe(true));
    let completeOld!: (v: SessionList) => void;
    vi.mocked(localCapture.sessions).mockReturnValueOnce(
      new Promise((resolve) => {
        completeOld = resolve;
      }),
    );
    act(() => result.current.selectProject(p1.id));
    await waitFor(() =>
      expect(localCapture.sessions).toHaveBeenCalledWith(p1.id),
    );
    act(() => result.current.selectProject(p2.id));
    await waitFor(() => expect(result.current.sessionsReady).toBe(true));
    act(() => result.current.selectProject(p1.id));
    await waitFor(() => expect(result.current.sessionsReady).toBe(true));
    await act(async () => {
      completeOld({ items: [], next: null });
    });
    expect(result.current.latest?.id).toBe(page.items[0]?.id);
  });
  it('does not read IPC or persist preferences in browser mode', async () => {
    const { result } = renderHook(() => useLocalWorkspace(false, true));
    await act(async () => {
      result.current.reachHome();
      await result.current.refresh();
    });
    expect(localCapture.projects).not.toHaveBeenCalled();
    expect(homeNative.read).not.toHaveBeenCalled();
    expect(homeNative.save).not.toHaveBeenCalled();
  });
  it('keeps the app usable when preferences cannot be loaded or saved', async () => {
    vi.mocked(homeNative.read).mockRejectedValue(
      new Error('private file diagnostic'),
    );
    vi.mocked(homeNative.save).mockRejectedValue(
      new Error('private write diagnostic'),
    );
    const { result } = renderHook(() => useLocalWorkspace(true, false));
    await waitFor(() => expect(result.current.preferencesReady).toBe(true));
    expect(result.current.preferenceNotice).toContain('You can continue');
    act(() => result.current.reachHome());
    await waitFor(() =>
      expect(result.current.preferenceNotice).toContain(
        'may reset after restart',
      ),
    );
    expect(result.current.loaded).toBe(true);
    expect(result.current.preferenceNotice).not.toContain('private');
  });
  it('clears a deleted current project on refresh and keeps failed reads explicit', async () => {
    const { result } = renderHook(() => useLocalWorkspace(true, false, false));
    await waitFor(() => expect(result.current.loaded).toBe(true));
    act(() => result.current.selectProject(p1.id));
    await waitFor(() => expect(result.current.sessionsReady).toBe(true));
    vi.mocked(localCapture.projects).mockResolvedValue([p2]);
    await act(async () => {
      await result.current.refresh();
    });
    expect(result.current.projectId).toBe('');
    expect(result.current.latest).toBeNull();
    vi.mocked(localCapture.projects).mockRejectedValue(
      new Error('private diagnostic'),
    );
    await act(async () => {
      await result.current.refresh();
    });
    expect(result.current.error).toContain('Local storage is unavailable');
    expect(result.current.error).not.toContain('private diagnostic');
  });
  it('uses one three-second polling cycle and suspends it while inactive', async () => {
    vi.useFakeTimers();
    try {
      const { result, rerender } = renderHook(
        ({ active }) => useLocalWorkspace(true, active, false),
        { initialProps: { active: true } },
      );
      await act(async () => {
        await vi.advanceTimersByTimeAsync(0);
      });
      expect(localCapture.projects).toHaveBeenCalledTimes(1);
      await act(async () => {
        await result.current.refresh();
        await result.current.refresh();
      });
      const count = vi.mocked(localCapture.projects).mock.calls.length;
      await act(async () => {
        await vi.advanceTimersByTimeAsync(6000);
      });
      expect(localCapture.projects).toHaveBeenCalledTimes(count + 2);
      rerender({ active: false });
      await act(async () => {
        await vi.advanceTimersByTimeAsync(0);
      });
      const inactiveCount = vi.mocked(localCapture.projects).mock.calls.length;
      await act(async () => {
        await vi.advanceTimersByTimeAsync(9000);
      });
      expect(localCapture.projects).toHaveBeenCalledTimes(inactiveCount);
    } finally {
      vi.useRealTimers();
    }
  });
});
