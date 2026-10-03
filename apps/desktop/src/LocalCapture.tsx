import { LearningPanel } from './Learning';
import { useEffect, useRef, useState } from 'react';
import { useLocalWorkspace } from './useLocalWorkspace';
import type { LocalWorkspace, SessionNavigation } from './useLocalWorkspace';
import { localCapture } from './native/local-capture';
import type { InstallPreview, SessionDetail } from './native/local-capture';

export function LocalCapture({
  enabled = true,
  approvalRequest = 0,
  workspace: shared,
  navigation,
}: {
  enabled?: boolean;
  approvalRequest?: number;
  workspace?: LocalWorkspace;
  navigation?: SessionNavigation | null;
}) {
  const deletePrompt = useRef<HTMLElement>(null);
  const approval = useRef<HTMLDetailsElement>(null);
  useEffect(() => {
    if (approvalRequest > 0 && approval.current) {
      approval.current.open = true;
      approval.current.querySelector('summary')?.focus();
    }
  }, [approvalRequest]);
  const ownWorkspace = useLocalWorkspace(enabled && !shared, true, false);
  const workspace = shared ?? ownWorkspace;
  const {
    projects,
    projectId,
    selectProject: setProjectId,
    sessions,
    setSessions,
    loaded,
    refresh,
    setBrowsingOlder,
  } = workspace;
  const connectionFocus = useRef<HTMLButtonElement>(null);
  const sessionFocus = useRef<HTMLHeadingElement>(null);
  const [focusSessionId, setFocusSessionId] = useState<string | null>(null);
  const busyNow = useRef(false);
  const [path, setPath] = useState('');
  const [name, setName] = useState('');
  const [rootConsent, setRootConsent] = useState(false);
  const [captureConsent, setCaptureConsent] = useState(false);
  const [preview, setPreview] = useState<InstallPreview | null>(null);
  const [detail, setDetail] = useState<SessionDetail | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState('');
  const [status, setStatus] = useState('');
  const [deleting, setDeleting] = useState<'project' | 'session' | null>(null);
  useEffect(() => {
    if (deleting) deletePrompt.current?.focus();
  }, [deleting]);
  const project = projects.find((p) => p.id === projectId);
  const captureStatus = workspace.status;
  const visibleStatus =
    status ||
    (captureStatus
      ? `${captureStatus.message}${
          captureStatus.spoolEvictionCount > 0
            ? ` Capture warning: ${captureStatus.spoolEvictionCount} queued records expired or exceeded local limits. Session coverage may be incomplete.`
            : ''
        }`
      : '');
  const visibleError = error || workspace.error;
  const [selection, setSelection] = useState(projectId);
  if (selection !== projectId) {
    setSelection(projectId);
    setCaptureConsent(false);
    setPreview(null);
    setDetail(null);
    setFocusSessionId(null);
    setDeleting(null);
    setStatus('');
    setError('');
  }
  async function loadDetail(
    sessionId: string,
    afterSequence: number | null = null,
  ) {
    const token = workspace.selectionToken();
    const result = await localCapture.detail(sessionId, afterSequence);
    const current =
      workspace.isCurrentSelection(token) &&
      workspace.isSelectedProject(projectId);
    if (current) setDetail(result);
    return current;
  }
  useEffect(() => {
    if (
      navigation?.target === 'session' &&
      focusSessionId &&
      detail?.session.id === focusSessionId
    )
      sessionFocus.current?.focus();
  }, [
    focusSessionId,
    detail?.session.id,
    navigation?.target,
    navigation?.token,
  ]);
  useEffect(() => {
    if (!navigation || !enabled) return;
    const timer = setTimeout(() => {
      if (navigation.target === 'approval') {
        setError('');
        setStatus('');
        if (navigation.draft) {
          setPath(navigation.draft.path);
          setName(navigation.draft.name);
          setRootConsent(false);
          setCaptureConsent(false);
          setPreview(null);
        }
        if (approval.current) {
          approval.current.open = true;
          approval.current.querySelector('input')?.focus();
        }
      } else if (navigation.projectId === projectId) {
        if (navigation.target === 'connection')
          connectionFocus.current?.focus();
        if (navigation.sessionId) {
          const token = workspace.selectionToken();
          void loadDetail(navigation.sessionId)
            .then((current) => {
              if (current) setFocusSessionId(navigation.sessionId ?? null);
            })
            .catch(() => {
              if (workspace.isCurrentSelection(token))
                setError(
                  'The session is unavailable. Retry from the session list.',
                );
            });
        }
      }
    }, 0);
    return () => clearTimeout(timer);
    // Navigation tokens represent deliberate one-time focus requests.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [navigation?.token, enabled]);
  async function action(run: () => Promise<unknown>) {
    if (!enabled || busyNow.current) return;
    busyNow.current = true;
    setBusy(true);
    setError('');
    const token = workspace.selectionToken();
    try {
      await run();
      await refresh();
    } catch {
      if (workspace.isCurrentSelection(token))
        setError(
          'The action could not be completed safely. Check project permissions, Codex CLI 0.151.0 and existing hooks, then retry.',
        );
    } finally {
      busyNow.current = false;
      setBusy(false);
    }
  }
  return (
    <section aria-labelledby="capture-title" className="local-capture">
      <div className="section-heading">
        <div>
          <h2 id="capture-title" className="sr-only">
            Your coding sessions
          </h2>
        </div>
        {projects.some((item) => item.tracking) && (
          <button
            disabled={busy || !enabled}
            onClick={() => void action(() => localCapture.pauseAll())}
          >
            Pause all capture
          </button>
        )}
      </div>
      {!loaded && !visibleError && <p role="status">Loading local projects…</p>}
      {visibleStatus && (
        <p role="status" className="capture-status">
          {visibleStatus}
        </p>
      )}
      {loaded && projects.length === 0 && !visibleError && (
        <section className="panel" aria-labelledby="empty-sessions-title">
          <h3 id="empty-sessions-title">No projects connected yet</h3>
          <p>Connect a project to see your next session here.</p>
          <a
            className="primary-link"
            href="#project-approval"
            onClick={() => {
              if (approval.current) approval.current.open = true;
            }}
          >
            Connect your first project <span aria-hidden="true">↗</span>
          </a>
        </section>
      )}
      {visibleError && (
        <p role="alert" className="error">
          {visibleError}
        </p>
      )}
      <details
        ref={approval}
        id="project-approval"
        className="project-approval"
      >
        <summary>Approve a project folder</summary>
        <fieldset disabled={!enabled || busy}>
          <form
            onSubmit={(e) => {
              e.preventDefault();
              if (!rootConsent) return;
              void action(async () => {
                const p = await localCapture.approve(path, name);
                setProjectId(p.id);
                workspace.setFolderDraft(null);
                setPath('');
                setName('');
                setRootConsent(false);
              });
            }}
          >
            <label>
              Project alias
              <input
                required
                maxLength={80}
                value={name}
                onChange={(e) => setName(e.target.value)}
                placeholder="My learning project"
              />
            </label>
            <label>
              Project folder
              <input
                required
                value={path}
                onChange={(e) => setPath(e.target.value)}
                placeholder="/Users/you/Projects/example"
              />
            </label>
            <label className="checkbox">
              <input
                type="checkbox"
                checked={rootConsent}
                onChange={(e) => setRootConsent(e.target.checked)}
              />
              I approve this folder as the project scope. Capture remains off
              until I approve the connection.
            </label>
            <button disabled={busy || !rootConsent} type="submit">
              Approve folder
            </button>
          </form>
        </fieldset>
      </details>
      {projects.length > 0 && (
        <label>
          Project
          <select
            disabled={busy || !enabled}
            value={projectId}
            onChange={(e) => {
              setBrowsingOlder(false);
              setProjectId(e.target.value);
              setSessions({ items: [], next: null });
              setCaptureConsent(false);
              setDetail(null);
              setPreview(null);
              setDeleting(null);
            }}
          >
            <option value="">Choose an approved project</option>
            {projects.map((p) => (
              <option key={p.id} value={p.id}>
                {p.name}
              </option>
            ))}
          </select>
        </label>
      )}
      {project && (
        <>
          <div className="project-controls">
            <p>
              <strong>
                {project.tracking
                  ? 'Local capture enabled'
                  : 'Local capture paused'}
              </strong>{' '}
            </p>
            <div className="actions">
              <button
                ref={connectionFocus}
                disabled={busy || !enabled}
                onClick={() =>
                  void action(async () => {
                    setCaptureConsent(false);
                    const token = workspace.selectionToken();
                    const next = await localCapture.preview(project.id);
                    if (
                      workspace.isCurrentSelection(token) &&
                      workspace.isSelectedProject(project.id)
                    )
                      setPreview(next);
                  })
                }
              >
                Review connection
              </button>
              <button
                disabled={busy || !enabled}
                onClick={() =>
                  void action(() =>
                    localCapture.tracking(project.id, !project.tracking),
                  )
                }
              >
                {project.tracking
                  ? 'Pause this project'
                  : 'Enable local capture'}
              </button>
            </div>
            <p className="muted">
              Enabling approves local tracking for this project. Approved
              capture can continue while mochi is closed; processing resumes
              when you reopen it.
            </p>
            <details className="project-details">
              <summary>Project details & disconnect</summary>
              <p className="muted">{project.root}</p>
              <p>Hook trust is reviewed separately in Codex /hooks.</p>
              <div className="actions">
                <button
                  disabled={busy || !enabled}
                  onClick={() =>
                    void action(async () => {
                      const token = workspace.selectionToken();
                      const next = await localCapture.preview(project.id, true);
                      if (
                        workspace.isCurrentSelection(token) &&
                        workspace.isSelectedProject(project.id)
                      )
                        setPreview(next);
                    })
                  }
                >
                  Review disconnect
                </button>
                <button
                  className="danger"
                  disabled={busy || !enabled}
                  onClick={() => setDeleting('project')}
                >
                  Delete project data
                </button>
              </div>
            </details>
          </div>
        </>
      )}
      {preview && (
        <section className="panel" aria-labelledby="connection-title">
          <h3 id="connection-title">Review the connection change</h3>
          <p>
            {preview.action === 'disconnect'
              ? 'Remove only the mochi-owned hooks and stop capture.'
              : 'Install project hooks that capture available Codex prompts, responses and activity into private local storage.'}
          </p>
          <p>
            Configuration file: <code>{preview.target}</code>
          </p>
          <details>
            <summary>Exact mochi command and events</summary>
            <pre>{preview.ownedCommand}</pre>
            <p>{preview.events.join(', ')}</p>
          </details>
          <p>
            Existing unrelated configuration is preserved. Recovery backups are
            private and expire at the next cleanup after seven days. Codex trust
            is never changed here.
          </p>
          {preview.action !== 'disconnect' && (
            <label className="checkbox">
              <input
                type="checkbox"
                checked={captureConsent}
                onChange={(e) => setCaptureConsent(e.target.checked)}
              />
              Enable local capture and approve this exact project connection. No
              remote analysis is authorized.
            </label>
          )}
          <div className="actions">
            <button
              disabled={
                busy || (preview.action !== 'disconnect' && !captureConsent)
              }
              onClick={() =>
                void action(async () => {
                  const token = workspace.selectionToken();
                  await localCapture.apply(preview.planId, captureConsent);
                  if (!workspace.isCurrentSelection(token)) return;
                  setPreview(null);
                  setStatus(
                    'Connection applied. Review the mochi hooks in Codex /hooks before coding.',
                  );
                })
              }
            >
              {preview.action === 'disconnect'
                ? 'Approve disconnect'
                : 'Approve connection'}
            </button>
            <button onClick={() => setPreview(null)}>Cancel</button>
          </div>
        </section>
      )}
      {project && (
        <div className="session-layout">
          <section className="session-library" aria-labelledby="sessions-title">
            <h3 id="sessions-title">Sessions</h3>
            {sessions.items.length === 0 ? (
              <p className="empty-state">
                No captured sessions yet. Connect the project, review mochi
                hooks in Codex, then work in the CLI as usual.
              </p>
            ) : (
              <ul className="session-list">
                {sessions.items.map((s) => (
                  <li key={s.id}>
                    <button
                      aria-pressed={detail?.session.id === s.id}
                      onClick={() => void action(async () => loadDetail(s.id))}
                    >
                      <time>{new Date(s.startedAt).toLocaleString()}</time>
                      <span>
                        {s.captureState} · {s.coverage} coverage
                      </span>
                    </button>
                  </li>
                ))}
              </ul>
            )}
            <div className="actions">
              <button
                disabled={busy || !enabled}
                onClick={() =>
                  void action(async () => {
                    const token = workspace.selectionToken();
                    setBrowsingOlder(false);
                    const page = await localCapture.sessions(project.id);
                    if (workspace.isCurrentSelection(token)) setSessions(page);
                  })
                }
              >
                Latest sessions
              </button>
              {sessions.next && (
                <button
                  disabled={busy || !enabled}
                  onClick={() =>
                    void action(async () => {
                      const token = workspace.selectionToken();
                      setBrowsingOlder(true);
                      const page = await localCapture.sessions(
                        project.id,
                        sessions.next,
                      );
                      if (workspace.isCurrentSelection(token))
                        setSessions(page);
                    })
                  }
                >
                  Older sessions
                </button>
              )}
            </div>
          </section>
          {!detail && (
            <section className="session-placeholder">
              <span aria-hidden="true">▤</span>
              <h3>Choose a session</h3>
              <p>
                Choose a captured session to explore its activity, code context
                and explanation.
              </p>
            </section>
          )}
          {detail && (
            <section className="panel" aria-labelledby="session-title">
              <h3 id="session-title" ref={sessionFocus} tabIndex={-1}>
                Observed session
              </h3>
              <p>
                {detail.session.captureState} · {detail.session.coverage}{' '}
                coverage · revision {detail.session.revision}
              </p>
              {detail.session.continuationOf && (
                <p>Continuation of an earlier work episode.</p>
              )}
              {detail.session.paused && (
                <p>Capture was paused. Some activity may be missing.</p>
              )}
              {detail.session.restarted && (
                <p>
                  mochi restarted during this episode. Its boundary is
                  uncertain.
                </p>
              )}
              {detail.session.lateEvidence && (
                <p>
                  Late evidence updated this episode. Any earlier explanation
                  needs a new review.
                </p>
              )}
              <div className="actions">
                <button
                  disabled={
                    busy ||
                    !enabled ||
                    detail.session.captureState === 'finalized'
                  }
                  onClick={() =>
                    void action(async () => {
                      await localCapture.finish(detail.session.id);
                      loadDetail(detail.session.id);
                    })
                  }
                >
                  Finish session
                </button>
                <button
                  disabled={busy || !enabled}
                  onClick={() =>
                    void action(async () => loadDetail(detail.session.id))
                  }
                >
                  Refresh details
                </button>
                <button
                  className="danger"
                  disabled={busy}
                  onClick={() => setDeleting('session')}
                >
                  Delete session
                </button>
              </div>
              <p className="muted">
                Finish ends mochi’s observed episode; it does not stop Codex.
              </p>
              <LearningPanel
                key={detail.session.id}
                sessionId={detail.session.id}
                finalized={detail.session.captureState === 'finalized'}
              />
              <details className="evidence-disclosure">
                <summary>Activity · {detail.eventCount} events</summary>
                <p>
                  {detail.eventCount} observed events. Missing results remain
                  unknown.
                </p>
                <ol className="events">
                  {detail.events.map((e) => (
                    <li key={e.id}>
                      <strong>{e.title}</strong>
                      {e.text && <pre>{e.text}</pre>}
                      {e.truncated && (
                        <p>
                          Display shortened; recorded evidence remains bounded.
                        </p>
                      )}
                    </li>
                  ))}
                </ol>
                <div className="actions">
                  <button
                    onClick={() =>
                      void action(async () => loadDetail(detail.session.id))
                    }
                  >
                    First events
                  </button>
                  {detail.nextSequence !== null && (
                    <button
                      onClick={() =>
                        void action(async () =>
                          loadDetail(detail.session.id, detail.nextSequence),
                        )
                      }
                    >
                      Next events
                    </button>
                  )}
                </div>
              </details>
              <details className="evidence-disclosure">
                <summary>Code context</summary>
                <p>{detail.gitNotice}</p>
                {detail.code.map((f) => (
                  <details key={f.path}>
                    <summary>{f.path}</summary>
                    {f.content !== null ? (
                      <pre>{f.content}</pre>
                    ) : (
                      <p>
                        Content was omitted by capture policy or unavailable.
                      </p>
                    )}
                    {f.omitted && <p>Some content is omitted.</p>}
                  </details>
                ))}
              </details>
            </section>
          )}
        </div>
      )}
      {deleting && (
        <section
          className="panel"
          ref={deletePrompt}
          tabIndex={-1}
          role="alertdialog"
          aria-labelledby="delete-title"
        >
          <h3 id="delete-title">
            Delete{' '}
            {deleting === 'project'
              ? 'this project and its sessions'
              : 'this session'}
            ?
          </h3>
          <p>
            Local captured content will be removed. Original project files and
            Codex history stay intact. Deletion does not erase provider
            submissions, backups or filesystem snapshots.
          </p>
          <div className="actions">
            <button
              disabled={busy || !enabled}
              onClick={() =>
                void action(async () => {
                  const token = workspace.selectionToken();
                  if (deleting === 'project' && project) {
                    await localCapture.deleteProject(project.id);
                    if (!workspace.isCurrentSelection(token)) return;
                    setProjectId('');
                  } else if (detail) {
                    await localCapture.deleteSession(detail.session.id);
                    if (!workspace.isCurrentSelection(token)) return;
                  }
                  setDetail(null);
                  setDeleting(null);
                })
              }
            >
              Confirm deletion
            </button>
            <button onClick={() => setDeleting(null)}>Keep data</button>
          </div>
        </section>
      )}
    </section>
  );
}
