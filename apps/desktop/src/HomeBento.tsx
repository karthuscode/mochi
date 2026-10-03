import type { RefObject } from 'react';
import { useRef, useState } from 'react';
import character from '../../../docs/design/assets/mochi/final/character-idle.png';
import { SpotlightCard } from './introduction/SpotlightCard';
import { homeNative } from './native/home';
import type {
  LocalWorkspace,
  ProjectFolderDraft,
  SessionNavigation,
} from './useLocalWorkspace';

type Source = 'codex-cli' | 'codex-desktop' | 'claude-code' | 'cowork';
const sources = [
  ['codex-cli', 'Codex CLI', 'Verified capture · 0.151.0'],
  [
    'codex-desktop',
    'Codex Desktop',
    'Not available yet · direct capture unverified',
  ],
  ['claude-code', 'Claude Code', 'Not available yet · compatibility research'],
  ['cowork', 'Claude Cowork', 'Not available yet · compatibility research'],
] as const;
export function HomeBento({
  native,
  workspace,
  onNavigate,
  onSettings,
  onReplay,
  onLegacyConnect,
  headingRef,
}: {
  native: boolean;
  workspace?: LocalWorkspace | undefined;
  onNavigate?:
    ((request: Omit<SessionNavigation, 'token'>) => void) | undefined;
  headingRef?: RefObject<HTMLHeadingElement | null>;
  onSettings: () => void;
  onReplay: () => void;
  onLegacyConnect: () => void;
}) {
  const [localDraft, setLocalDraft] = useState<ProjectFolderDraft | null>(null);
  const draft = workspace ? workspace.folderDraft : localDraft;
  const setDraft = workspace ? workspace.setFolderDraft : setLocalDraft;
  const [source, setSource] = useState<Source | ''>('');
  const [picking, setPicking] = useState(false);
  const pickingNow = useRef(false);
  const [pickerError, setPickerError] = useState('');
  const [help, setHelp] = useState(false);
  const projectSelect = useRef<HTMLSelectElement>(null);
  const sourceFocus = useRef<HTMLFieldSetElement>(null);
  const project = !draft
    ? workspace?.projects.find((p) => p.id === workspace.projectId)
    : undefined;
  const latest = project ? workspace?.latest : null;
  const lesson = workspace?.lesson;
  const validLesson =
    latest &&
    lesson?.sessionId === latest.id &&
    !lesson.stale &&
    lesson.inputRevision === latest.revision &&
    lesson.status === 'internal_explanation';
  const navigate = (target: 'connection' | 'session', sessionId?: string) => {
    if (native && project)
      onNavigate?.({
        target,
        projectId: project.id,
        ...(sessionId ? { sessionId } : {}),
      });
  };
  async function pickFolder() {
    if (!native || pickingNow.current) return;
    pickingNow.current = true;
    setPicking(true);
    setPickerError('');
    try {
      const path = await homeNative.pickFolder();
      if (path !== null) {
        const approved = workspace?.projects.find((p) => p.root === path);
        if (approved && workspace) {
          setDraft(null);
          setSource('');
          workspace.selectProject(approved.id);
          return;
        }
        setDraft({
          path,
          name: (
            path.split('/').filter(Boolean).at(-1) ?? 'My learning project'
          ).slice(0, 80),
        });
        setSource('');
      }
    } catch {
      setPickerError(
        'The folder picker is unavailable. Try again or enter a folder in Sessions.',
      );
    } finally {
      pickingNow.current = false;
      setPicking(false);
    }
  }
  let title = 'Choose where you want to learn.';
  let text = 'Start with one project. You can add others later.';
  let action = 'Choose a project folder';
  let run = () => {
    if (workspace?.projects.length) projectSelect.current?.focus();
    else void pickFolder();
  };
  let disabled = !native || picking;
  if (draft) {
    if (!source) {
      title = 'Which tool do you use here?';
      text = 'Choose the coding tool you use with this project.';
      action = 'Choose your coding tool';
      run = () =>
        sourceFocus.current?.querySelector<HTMLInputElement>('input')?.focus();
    } else if (source !== 'codex-cli') {
      title = 'This connection is not available yet.';
      text =
        'We’re checking capture and privacy for this tool. You can choose another tool or return to your projects.';
      action = 'Choose another tool';
      run = () => {
        setSource('');
        sourceFocus.current?.querySelector<HTMLInputElement>('input')?.focus();
      };
    } else {
      title = 'Review your project folder.';
      text =
        'Next, approve the folder in Sessions. This selection has not enabled capture.';
      action = 'Review project access';
      disabled ||= !draft.name.trim();
      run = () => onNavigate?.({ target: 'approval', draft });
    }
  } else if (project) {
    if (!project.tracking) {
      title = 'Review your connection.';
      text =
        'Local capture is paused. Review the connection and Codex hook trust before enabling capture.';
      action = 'Review connection';
      run = () => navigate('connection');
    } else if (!workspace?.sessionsReady) {
      title = 'Checking your sessions…';
      text = 'Your next step will appear when local history loads.';
      action = 'Loading…';
      disabled = true;
    } else if (!latest) {
      title = 'Make your first small change.';
      text =
        'Review mochi’s hooks in Codex /hooks, then work in Codex CLI as usual. Only new work is captured; we’re waiting for its first session.';
      action = 'Open sessions';
      run = () => navigate('session');
    } else if (latest.captureState !== 'finalized') {
      title = 'Look back at your session.';
      text =
        'Explore the recorded activity. You can finish mochi’s episode when ready; this does not stop Codex.';
      action = 'Open session';
      run = () => navigate('session', latest.id);
    } else if (validLesson) {
      title = 'Understand what you built.';
      text =
        'Open your saved explanation and try its self-check. This is an internal learning preview.';
      action = 'Open explanation & self-check';
      run = () => navigate('session', latest.id);
    } else if (workspace?.analysis?.running) {
      title = 'Your analysis is in progress.';
      text =
        'Open the session to check progress or cancel the current analysis.';
      action = 'Open session';
      run = () => navigate('session', latest.id);
    } else if (workspace?.learningError || !workspace?.analysis) {
      title = 'Check your learning setup.';
      text =
        workspace?.learningError || 'Learning settings could not be checked.';
      action = 'Open learning settings';
      run = onSettings;
    } else if (!workspace.analysis.keyConfigured) {
      title = 'Set up optional explanations.';
      text =
        'New explanations use your own OpenAI API key and separately billed API usage. Your session is already saved locally.';
      action = 'Open learning settings';
      run = onSettings;
    } else if (!workspace.analysis.enabled) {
      title = 'Choose whether to analyze.';
      text =
        'Remote analysis is off. Enable its controls in Settings, then review the exact session context before any send.';
      action = 'Open learning settings';
      run = onSettings;
    } else {
      title =
        lesson?.status === 'insufficient_context' && !lesson.stale
          ? 'Review missing context.'
          : lesson?.stale || (lesson && !validLesson)
            ? 'Review the latest session evidence.'
            : 'Review your analysis context.';
      text =
        lesson?.status === 'insufficient_context' && !lesson.stale
          ? 'The last analysis did not have enough safe context for an explanation. Open the session to inspect the omissions and next steps.'
          : 'Open the session to inspect the exact context and approve a send. Nothing is uploaded from Home.';
      action = 'Open session';
      run = () => navigate('session', latest.id);
    }
  }
  if (workspace?.error || (workspace && !workspace.loaded)) disabled = true;
  return (
    <section className="bento-home" aria-labelledby="bento-title">
      <div className="bento-heading">
        <div>
          <p className="welcome-kicker">Your work. Your next step.</p>
          <h2 id="bento-title" ref={headingRef} tabIndex={-1}>
            Make more of what you build.
          </h2>
        </div>
        <button className="quiet-button" onClick={onReplay}>
          Replay introduction
        </button>
      </div>
      {workspace?.preferenceNotice && (
        <p role="status" className="bento-notice">
          {workspace.preferenceNotice}
        </p>
      )}
      {workspace?.error && (
        <div role="alert" className="bento-notice">
          <p>{workspace.error}</p>
          <button onClick={() => void workspace.refresh()}>
            Retry local data
          </button>
        </div>
      )}
      {workspace && !workspace.loaded && !workspace.error && (
        <p role="status">Loading local projects…</p>
      )}
      <div className="bento-grid">
        <SpotlightCard
          className="bento-project"
          spotlightColor="rgba(150, 110, 240, 0.12)"
        >
          <p className="bento-kicker">01 · YOUR PROJECT</p>
          <div className="bento-project-heading">
            <div>
              <h3>
                {draft
                  ? 'Your next learning project'
                  : (project?.name ?? 'Start with your own project')}
              </h3>
              <p>
                {project
                  ? 'Codex CLI · approved project'
                  : 'Choose the folder you work in. mochi follows only projects you approve.'}
              </p>
            </div>
            <img src={character} alt="" className="bento-character" />
          </div>
          {!!workspace?.projects.length && (
            <label>
              Learning project
              <select
                ref={projectSelect}
                value={draft ? '' : workspace.projectId}
                disabled={!native || picking}
                onChange={(e) => {
                  setDraft(null);
                  setSource('');
                  workspace.selectProject(e.target.value);
                }}
              >
                <option value="">Choose an approved project</option>
                {[...workspace.projects]
                  .sort((a, b) => a.name.localeCompare(b.name))
                  .map((p) => (
                    <option key={p.id} value={p.id}>
                      {p.name}
                    </option>
                  ))}
              </select>
            </label>
          )}
          {draft && (
            <div className="bento-draft">
              <p className="bento-path">{draft.path}</p>
              <label>
                Project name
                <input
                  maxLength={80}
                  value={draft.name}
                  onChange={(e) => setDraft({ ...draft, name: e.target.value })}
                />
              </label>
              <fieldset ref={sourceFocus} className="bento-sources">
                <legend>Which coding tool do you use?</legend>
                {sources.map(([id, label, status]) => (
                  <label key={id}>
                    <input
                      type="radio"
                      name="bento-source"
                      checked={source === id}
                      onChange={() => setSource(id)}
                    />
                    <span>
                      <strong>{label}</strong>
                      <small>{status}</small>
                    </span>
                  </label>
                ))}
              </fieldset>
              <button
                className="quiet-button"
                onClick={() => {
                  setDraft(null);
                  setSource('');
                }}
              >
                Back to projects
              </button>
            </div>
          )}
          <div className="bento-actions">
            <button
              disabled={
                !native ||
                picking ||
                !!workspace?.error ||
                (workspace && !workspace.loaded)
              }
              onClick={() => void pickFolder()}
            >
              {picking ? 'Opening folder picker…' : 'Choose a project folder'}
            </button>
          </div>
          <p className="bento-footnote">
            Choosing a folder grants no capture or upload permission.
          </p>
          {pickerError && (
            <div role="alert">
              <p>{pickerError}</p>
              <button onClick={onLegacyConnect}>
                Enter folder in Sessions
              </button>
            </div>
          )}
          {!native && (
            <p className="bento-footnote">
              Project selection requires the macOS app. No data is loaded in
              this appearance preview.
            </p>
          )}
        </SpotlightCard>
        <SpotlightCard
          className="bento-next"
          spotlightColor="rgba(255, 112, 72, 0.12)"
        >
          <p className="bento-kicker">02 · NEXT STEP</p>
          <h3>{title}</h3>
          <p>{text}</p>
          <button className="primary-link" disabled={disabled} onClick={run}>
            {action} <span aria-hidden="true">→</span>
          </button>
          <ol className="bento-route" aria-label="Your project journey">
            <li>Choose project</li>
            <li>Review connection</li>
            <li>Revisit work</li>
            <li>Try a self-check</li>
          </ol>
        </SpotlightCard>
        <SpotlightCard
          className="bento-history"
          spotlightColor="rgba(150, 110, 240, 0.10)"
        >
          <p className="bento-kicker">
            03 · {latest ? 'LATEST SESSION' : 'A LITTLE GUIDANCE'}
          </p>
          {latest ? (
            <>
              <h3>Your most recent work</h3>
              <p>
                <time dateTime={latest.startedAt}>
                  {new Date(latest.startedAt).toLocaleString()}
                </time>
              </p>
              <p className="bento-session-status">
                {latest.captureState} · {latest.coverage} capture
              </p>
              {latest.coverage !== 'complete' && (
                <p>Some activity may be missing.</p>
              )}
              {(latest.restarted || latest.paused || latest.lateEvidence) && (
                <p>
                  Capture has gaps or updated evidence. Inspect the session
                  before drawing conclusions.
                </p>
              )}
              <button
                disabled={!native}
                onClick={() => navigate('session', latest.id)}
              >
                View session →
              </button>
            </>
          ) : (
            <>
              <h3>One step at a time.</h3>
              <p>
                Choose a project, review its connection, then code in your usual
                tool. Return here to explore what happened.
              </p>
            </>
          )}
          <button
            className="quiet-button"
            aria-expanded={help}
            onClick={() => setHelp(!help)}
          >
            {help ? 'Hide first-session guide' : 'Show first-session guide'}
          </button>
          {help && (
            <ol className="bento-help">
              <li>Choose and approve one project folder.</li>
              <li>
                Review the exact connection, then mochi’s hooks in Codex /hooks.
              </li>
              <li>
                Make a small change in Codex CLI. Only new work is captured.
              </li>
              <li>
                Inspect the session and finish mochi’s episode when ready.
              </li>
              <li>
                Optionally configure your API key, review context and approve
                analysis.
              </li>
              <li>Read the explanation and try a self-check.</li>
            </ol>
          )}
        </SpotlightCard>
        <SpotlightCard
          className="bento-privacy"
          spotlightColor="rgba(150, 110, 240, 0.10)"
        >
          <p className="bento-kicker">04 · CONNECTION & PRIVACY</p>
          <h3>You stay in control.</h3>
          <dl>
            <div>
              <dt>Local capture</dt>
              <dd>
                {draft || !project
                  ? 'No project selected'
                  : project.tracking
                    ? 'Enabled for this project'
                    : 'Paused for this project'}
              </dd>
            </div>
            <div>
              <dt>Remote analysis</dt>
              <dd>
                {workspace?.analysis
                  ? workspace.analysis.enabled
                    ? 'Controls enabled · exact approval required'
                    : 'Off'
                  : native
                    ? 'Status unavailable'
                    : 'Unavailable in browser preview'}
              </dd>
            </div>
          </dl>
          <p>
            Your sessions stay on this Mac. New explanations require an API key
            and a separately approved send.
          </p>
          {!!workspace?.status?.spoolEvictionCount && (
            <p>
              Some queued capture records expired or exceeded local limits.
              Coverage may be incomplete.
            </p>
          )}
          <button onClick={onSettings}>Open learning settings →</button>
        </SpotlightCard>
      </div>
    </section>
  );
}
