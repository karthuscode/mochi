import { useCallback, useEffect, useRef, useState } from 'react';
import { localCapture } from './native/local-capture';
import type {
  LocalStatus,
  ProjectView,
  SessionList,
  SessionView,
} from './native/local-capture';
import { learning } from './native/learning';
import type { AnalysisStatus, Lesson } from './native/learning';
import { defaultHomePreferences, homeNative } from './native/home';
import type { HomePreferences } from './native/home';

export interface SessionNavigation {
  token: number;
  projectId?: string;
  sessionId?: string;
  target: 'approval' | 'connection' | 'session';
  draft?: { path: string; name: string };
}
const emptySessions: SessionList = { items: [], next: null };
export type ProjectFolderDraft = { path: string; name: string };

// A window-scoped read/selection owner. Mutations remain in their existing consent UI.
export function useLocalWorkspace(
  enabled: boolean,
  active: boolean,
  remember = true,
) {
  const [projects, setProjects] = useState<ProjectView[]>([]);
  const [projectId, setProjectId] = useState('');
  const [folderDraft, setFolderDraft] = useState<ProjectFolderDraft | null>(
    null,
  );
  const selected = useRef('');
  const selectionEpoch = useRef(0);
  const [sessions, setSessions] = useState<SessionList>(emptySessions);
  const [latest, setLatest] = useState<SessionView | null>(null);
  const [sessionsReady, setSessionsReady] = useState(false);
  const [loaded, setLoaded] = useState(!enabled);
  const [error, setError] = useState('');
  const [status, setStatus] = useState<LocalStatus | null>(null);
  const [analysis, setAnalysis] = useState<AnalysisStatus | null>(null);
  const [lesson, setLesson] = useState<Lesson | null>(null);
  const [learningError, setLearningError] = useState('');
  const [preferences, setPreferences] = useState<HomePreferences>(
    defaultHomePreferences,
  );
  const preferencesRef = useRef(defaultHomePreferences);
  const [preferencesReady, setPreferencesReady] = useState(
    !enabled || !remember,
  );
  const [preferenceNotice, setPreferenceNotice] = useState('');
  const initialized = useRef(!remember);
  const browsingOlder = useRef(false);
  const sequence = useRef(0);
  const inflight = useRef<{ id: number; project: string } | null>(null);
  const mounted = useRef(false);
  const saveState = useRef<{
    saving: boolean;
    pending: HomePreferences | null;
  }>({ saving: false, pending: null });

  const savePreferences = useCallback(
    (next: HomePreferences) => {
      preferencesRef.current = next;
      setPreferences(next);
      if (!enabled || !remember) return;
      saveState.current.pending = next;
      if (saveState.current.saving) return;
      saveState.current.saving = true;
      void (async () => {
        while (saveState.current.pending) {
          const pending = saveState.current.pending;
          saveState.current.pending = null;
          try {
            await homeNative.save(pending);
            if (mounted.current) setPreferenceNotice('');
          } catch {
            if (mounted.current)
              setPreferenceNotice(
                'Your Home choice could not be saved. It may reset after restart.',
              );
          }
        }
        saveState.current.saving = false;
      })();
    },
    [enabled, remember],
  );

  const selectProject = useCallback(
    (id: string) => {
      if (selected.current === id) return;
      selected.current = id;
      selectionEpoch.current++;
      sequence.current++;
      setProjectId(id);
      setSessions(emptySessions);
      setLatest(null);
      setLesson(null);
      setSessionsReady(false);
      setLearningError('');
      browsingOlder.current = false;
      if (initialized.current)
        savePreferences({ ...preferencesRef.current, projectId: id || null });
    },
    [savePreferences],
  );

  const refresh = useCallback(async () => {
    if (!enabled) return;
    const project = selected.current;
    if (
      inflight.current?.project === project &&
      inflight.current.id === sequence.current
    )
      return;
    const id = ++sequence.current;
    inflight.current = { id, project };
    const current = () =>
      mounted.current &&
      sequence.current === id &&
      selected.current === project;
    try {
      const rows: ProjectView[] = [];
      let after: string | null = null;
      // Reuse the bounded, paginated project API; never scan the user's folders.
      for (let page = 0; page < 20; page++) {
        const batch = await localCapture.projects(after);
        rows.push(...batch);
        if (batch.length < 50) break;
        after = batch.at(-1)?.id ?? null;
        if (page === 19) throw Error('Project list limit reached.');
      }
      const state = await localCapture.status();
      if (!current()) return;
      setProjects(rows);
      setStatus(state);
      setLoaded(true);
      setError('');
      if (!initialized.current) {
        let saved = defaultHomePreferences;
        try {
          saved = await homeNative.read();
        } catch {
          if (current())
            setPreferenceNotice(
              'Home preferences are unavailable. You can continue without them.',
            );
        }
        if (!current()) return;
        initialized.current = true;
        const restored = rows.some((p) => p.id === saved.projectId)
          ? saved.projectId
          : null;
        const next = {
          ...saved,
          homeReached: saved.homeReached || rows.length > 0,
          projectId: restored,
        };
        preferencesRef.current = next;
        setPreferences(next);
        setPreferencesReady(true);
        if (restored) {
          selectProject(restored);
          return;
        }
      }
      if (project && !rows.some((p) => p.id === project)) {
        selectProject('');
        return;
      }
      let analysisState: AnalysisStatus | null = null;
      try {
        analysisState = await learning.status();
      } catch {
        if (current())
          setLearningError(
            'Learning status is unavailable. Open Settings to check it.',
          );
      }
      if (!current()) return;
      setAnalysis(analysisState);
      if (!project) return;
      const page = await localCapture.sessions(project);
      if (!current()) return;
      const newest = page.items[0] ?? null;
      setLatest(newest);
      setSessionsReady(true);
      if (!browsingOlder.current) setSessions(page);
      let document: Lesson | null = null;
      try {
        if (newest?.captureState === 'finalized')
          document = await learning.lesson(newest.id);
        if (current() && analysisState) setLearningError('');
      } catch {
        if (current())
          setLearningError(
            'Explanation status is unavailable. Open the session to check it.',
          );
      }
      if (current()) setLesson(document);
    } catch {
      if (current())
        setError(
          'Local storage is unavailable. Retry or restart mochi and check its permissions.',
        );
    } finally {
      if (inflight.current?.id === id) inflight.current = null;
    }
  }, [enabled, selectProject]);

  useEffect(() => {
    mounted.current = true;
    return () => {
      mounted.current = false;
      sequence.current++;
    };
  }, []);
  useEffect(() => {
    if (!enabled) return;
    const first = setTimeout(() => {
      void refresh();
    }, 0);
    const timer = active
      ? setInterval(() => {
          void refresh();
        }, 3000)
      : null;
    return () => {
      clearTimeout(first);
      if (timer !== null) clearInterval(timer);
    };
  }, [enabled, active, projectId, refresh]);

  const reachHome = useCallback(() => {
    savePreferences({ ...preferencesRef.current, homeReached: true });
  }, [savePreferences]);
  return {
    projects,
    projectId,
    folderDraft,
    setFolderDraft,
    selectProject,
    sessions,
    setSessions,
    latest,
    sessionsReady,
    loaded,
    error,
    status,
    analysis,
    lesson,
    learningError,
    refresh,
    setBrowsingOlder: (older: boolean) => {
      browsingOlder.current = older;
    },
    isSelectedProject: (id: string) => selected.current === id,
    selectionToken: () => selectionEpoch.current,
    isCurrentSelection: (token: number) => selectionEpoch.current === token,
    preferences,
    preferencesReady,
    preferenceNotice,
    reachHome,
  };
}
export type LocalWorkspace = ReturnType<typeof useLocalWorkspace>;
