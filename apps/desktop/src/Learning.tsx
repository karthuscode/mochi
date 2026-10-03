import { useCallback, useEffect, useRef, useState } from 'react';
import { learning } from './native/learning';
import type {
  AnalysisStatus,
  Lesson,
  SendPreview,
  Question,
  Attempt,
} from './native/learning';
export function LearningSettings() {
  const [status, setStatus] = useState<AnalysisStatus | null>(null);
  const [error, setError] = useState('');
  const key = useRef<HTMLInputElement>(null);
  const [busy, setBusy] = useState(false);
  const refresh = useCallback(async () => {
    setStatus(await learning.status());
  }, []);
  useEffect(() => {
    let live = true;
    const update = () => {
      void learning
        .status()
        .then((s) => {
          if (live) setStatus(s);
        })
        .catch(() => {
          if (live)
            setError(
              'Analysis settings are unavailable. Check local Keychain access.',
            );
        });
    };
    const initial = setTimeout(update, 0);
    const timer = setInterval(update, 2000);
    return () => {
      live = false;
      clearTimeout(initial);
      clearInterval(timer);
    };
  }, []);
  async function action(run: () => Promise<void>) {
    setBusy(true);
    setError('');
    try {
      await run();
      await refresh();
    } catch (e) {
      setError(
        e instanceof Error ? e.message : 'Analysis settings unavailable.',
      );
    } finally {
      setBusy(false);
    }
  }
  return (
    <section className="panel" aria-labelledby="analysis-settings-title">
      <h2 id="analysis-settings-title" tabIndex={-1}>
        Learning settings
      </h2>
      <p>
        OpenAI API usage is billed separately from ChatGPT. Your API key stays
        in macOS Keychain; mochi never reads your Codex credentials.
      </p>
      <p role="status">{status?.message}</p>
      {error && <p role="alert">{error}</p>}
      <form
        onSubmit={(e) => {
          e.preventDefault();
          const input = key.current;
          if (!input) return;
          const value = input.value;
          input.value = '';
          void action(() => learning.setKey(value));
        }}
      >
        <label>
          OpenAI API key
          <input
            ref={key}
            type="password"
            autoComplete="off"
            spellCheck={false}
            maxLength={512}
            required
            placeholder={
              status?.keyConfigured ? 'Key stored in Keychain' : 'sk-…'
            }
          />
        </label>
        <button disabled={busy} type="submit">
          Store in Keychain
        </button>
      </form>
      <button
        disabled={busy || !status?.keyConfigured}
        onClick={() => void action(() => learning.deleteKey())}
      >
        Remove API key and disable remote analysis
      </button>
      <label className="checkbox">
        <input
          type="checkbox"
          checked={status?.enabled ?? false}
          disabled={busy}
          onChange={(e) => {
            const enabled = e.target.checked;
            void action(() => learning.permission(enabled));
          }}
        />
        Allow remote analysis controls for this app run. Every session or answer
        still requires an exact send approval.
      </label>
      <p className="muted">
        No content is sent by opening mochi or enabling this control. Permission
        resets on restart.
      </p>
      <details>
        <summary>Provider details</summary>
        <p>
          Model: {status?.model ?? 'gpt-4o-mini-2024-07-18'}. Requests use
          OpenAI with storage disabled; provider retention policies still apply.
        </p>
      </details>
      {status?.running && (
        <button onClick={() => void action(() => learning.cancel())}>
          Cancel running analysis
        </button>
      )}
    </section>
  );
}
export function LearningPanel({
  sessionId,
  finalized,
}: {
  sessionId: string;
  finalized: boolean;
}) {
  const [lesson, setLesson] = useState<Lesson | null>(null);
  const [preview, setPreview] = useState<SendPreview | null>(null);
  const [consent, setConsent] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState('');
  const [message, setMessage] = useState('');
  const refresh = useCallback(async () => {
    setLesson(await learning.lesson(sessionId));
  }, [sessionId]);
  useEffect(() => {
    let live = true;
    const update = () => {
      void learning
        .lesson(sessionId)
        .then((v) => {
          if (live) setLesson(v);
        })
        .catch(() => {
          if (live) setError('Saved explanation is unavailable.');
        });
    };
    const initial = setTimeout(update, 0);
    const timer = setInterval(update, 2500);
    return () => {
      live = false;
      clearTimeout(initial);
      clearInterval(timer);
    };
  }, [sessionId]);
  async function action(run: () => Promise<void>) {
    if (busy) return;
    setBusy(true);
    setError('');
    try {
      await run();
    } catch (e) {
      setError(e instanceof Error ? e.message : 'Learning action unavailable.');
    } finally {
      setBusy(false);
    }
  }
  async function prepare(attemptId: string | null = null) {
    setConsent(false);
    setPreview(await learning.preview(sessionId, attemptId));
  }
  function showEvidence(reference: string) {
    const target = document.getElementById(
      `evidence-${sessionId}-${reference}`,
    );
    if (!target) return;
    let container = target.parentElement;
    while (container) {
      if (container instanceof HTMLDetailsElement) container.open = true;
      container = container.parentElement;
    }
    target.focus();
  }
  return (
    <section className="learning" aria-labelledby="learning-title">
      <h4 id="learning-title">Learn from this work</h4>
      {!finalized && (
        <p>
          Finish this observed session to prepare a stable analysis preview.
          Ending one Codex turn does not finish the whole work episode.
        </p>
      )}
      <button
        disabled={busy || !finalized}
        onClick={() => void action(() => prepare())}
      >
        Preview explanation request
      </button>
      <p role="status">{message}</p>
      {error && <p role="alert">{error}</p>}
      {preview && (
        <section className="panel" aria-labelledby="send-title">
          <h5 id="send-title">Review exactly what will be sent</h5>
          <p>
            {preview.purpose === 'grading'
              ? 'This sends your saved answer and its question/rubric for advisory evaluation.'
              : 'This sends selected sanitized activity and code evidence to generate an explanation and checks.'}
          </p>
          <p>
            Provider: {preview.provider} · Model: {preview.model} · Revision{' '}
            {preview.inputRevision}
          </p>
          <p>
            Endpoint: <code>{preview.endpoint}</code>
          </p>
          <p className="muted">
            Request fingerprint: <code>{preview.requestHash}</code>. Approval
            expires in five minutes and after permission or input changes.
          </p>
          <details>
            <summary>
              Complete request, evidence, instructions and response schema
            </summary>
            <pre>
              {JSON.stringify(JSON.parse(preview.requestJson), null, 2)}
            </pre>
          </details>
          <p>
            Review for private information that pattern detection may miss. No
            tools or code execution are enabled. Storage is disabled in this API
            request; OpenAI's retention rules still apply and already submitted
            data cannot be recalled by deleting it locally.
          </p>
          <label className="checkbox">
            <input
              checked={consent}
              type="checkbox"
              onChange={(e) => setConsent(e.target.checked)}
            />
            I approve sending this exact{' '}
            {preview.purpose === 'grading'
              ? 'answer and rubric'
              : 'session request'}{' '}
            to OpenAI. This may incur API charges.
          </label>
          <div className="actions">
            <button
              disabled={busy || !consent}
              onClick={() =>
                void action(async () => {
                  await learning.send(preview.token);
                  setPreview(null);
                  setConsent(false);
                  setMessage(
                    'Request approved. Progress and cancellation are available in Learning settings.',
                  );
                })
              }
            >
              Send approved request
            </button>
            <button
              onClick={() => {
                setPreview(null);
                setConsent(false);
              }}
            >
              Keep local
            </button>
          </div>
        </section>
      )}
      {lesson && (
        <article aria-labelledby="explanation-title">
          <h5 id="explanation-title">
            {lesson.status === 'insufficient_context'
              ? 'Insufficient context'
              : 'Explanation and self-check'}
          </h5>
          <p>
            Internal test explanation · {lesson.coverage} coverage · source
            revision {lesson.inputRevision}
          </p>
          {lesson.stale && (
            <p role="alert">
              Evidence, exclusions or project policy changed. This is an earlier
              explanation; preview a fresh request before continuing these
              checks.
            </p>
          )}
          <p>
            AI-generated explanations can be wrong. Citations identify the
            captured evidence, and inferred reasoning remains an inference. This
            is not a complete V1 lesson or proof of understanding.
          </p>
          {(
            [
              'overview',
              'built',
              'changed',
              'decisions',
              'failures',
              'unresolved',
            ] as const
          ).map((section, i) => (
            <section key={section}>
              <h6>
                {
                  [
                    'Session overview',
                    'Requested and observed work',
                    'What changed',
                    'Decisions and alternatives',
                    'Failures and fixes',
                    'What remains unknown',
                  ][i]
                }
              </h6>
              {lesson.reconstruction[section].map((claim, j) => (
                <p key={j}>
                  <span className="confidence">{claim.confidence}</span>{' '}
                  {claim.text}{' '}
                  {claim.references.map((r) => (
                    <a
                      key={r}
                      href={`#evidence-${sessionId}-${r}`}
                      onClick={() => showEvidence(r)}
                    >
                      [evidence]
                    </a>
                  ))}
                </p>
              ))}
            </section>
          ))}
          {lesson.cards.map((card) => (
            <section className="panel" key={card.key}>
              <h6>{card.title}</h6>
              <p>{card.definition}</p>
              {card.codeQuote && (
                <>
                  <p>From the sanitized project snapshot:</p>
                  <pre>{card.codeQuote}</pre>
                </>
              )}
              <p>
                <strong>Why it works.</strong> {card.whyItWorks}
              </p>
              <p>
                <strong>Why it matters.</strong> {card.whyItMatters}
              </p>
              <p>
                <strong>Common misconception.</strong> {card.misconception}
              </p>
              <p>
                {card.references.map((r) => (
                  <a
                    key={r}
                    href={`#evidence-${sessionId}-${r}`}
                    onClick={() => showEvidence(r)}
                  >
                    Project evidence{' '}
                  </a>
                ))}
              </p>
              <details>
                <summary>Why this concept was selected</summary>
                <p>
                  Selection score {card.selectionScore}; novelty{' '}
                  {card.components.novelty}, relevance{' '}
                  {card.components.relevance}, importance{' '}
                  {card.components.importance}, impact {card.components.impact},
                  weakness {card.components.weakness}. These rank teaching
                  opportunities and do not measure your mastery.
                </p>
              </details>
              <SelfCheck
                key={card.question.id}
                question={card.question}
                stale={lesson.stale}
                onSaved={refresh}
                onGrade={(id) => action(() => prepare(id))}
              />
            </section>
          ))}
          <details>
            <summary>Omissions and source snapshots</summary>
            <ul>
              {lesson.omissions.map((o) => (
                <li key={o}>{o}</li>
              ))}
            </ul>
            {lesson.evidence.map((e) => (
              <section
                tabIndex={-1}
                id={`evidence-${sessionId}-${e.id}`}
                key={e.id}
              >
                <h6>{e.path ?? e.kind}</h6>
                <p>
                  Captured {e.capturedAt}
                  {e.firstLine !== null
                    ? ` · starting line ${e.firstLine}`
                    : ''}
                  {e.truncated ? ' · excerpt shortened' : ''}
                </p>
                <pre>{e.text}</pre>
              </section>
            ))}
          </details>
        </article>
      )}
    </section>
  );
}
function SelfCheck({
  question,
  stale,
  onSaved,
  onGrade,
}: {
  question: Question;
  stale: boolean;
  onSaved: () => Promise<void>;
  onGrade: (id: string) => Promise<void>;
}) {
  const [answer, setAnswer] = useState('');
  const [assistance, setAssistance] = useState('unknown');
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState('');
  const [revealed, setRevealed] = useState('');
  const attemptToken = useRef<string | null>(null);
  const [saved, setSaved] = useState<Attempt | null>(null);
  const latest = saved ?? question.attempts[0];
  async function submit() {
    if (busy || stale) return;
    setBusy(true);
    setError('');
    attemptToken.current ??= crypto.randomUUID();
    try {
      const attempt = await learning.submit(
        attemptToken.current,
        question.id,
        answer,
        assistance,
      );
      setSaved(attempt);
      attemptToken.current = null;
      setAnswer('');
      await onSaved();
    } catch (e) {
      setError(e instanceof Error ? e.message : 'Answer unavailable.');
    } finally {
      setBusy(false);
    }
  }
  return (
    <section aria-labelledby={`question-${question.id}`}>
      <h6 id={`question-${question.id}`}>Self-check</h6>
      <p>{question.prompt}</p>
      {question.snippet && (
        <>
          <p>Practice example; this code is never executed by mochi.</p>
          <pre>{question.snippet}</pre>
          <p>{question.assumptions}</p>
        </>
      )}
      <form
        onSubmit={(e) => {
          e.preventDefault();
          void submit();
        }}
      >
        {question.kind === 'mcq' ? (
          <fieldset disabled={busy || stale}>
            <legend>Your choice</legend>
            {question.choices.map((c) => (
              <label className="checkbox" key={c.id}>
                <input
                  type="radio"
                  name={question.id}
                  value={c.id}
                  checked={answer === c.id}
                  onChange={() => {
                    setAnswer(c.id);
                    attemptToken.current = null;
                  }}
                />
                {c.text}
              </label>
            ))}
          </fieldset>
        ) : (
          <label>
            Your answer
            <textarea
              maxLength={16384}
              disabled={busy || stale}
              value={answer}
              onChange={(e) => {
                setAnswer(e.target.value);
                attemptToken.current = null;
              }}
            />
          </label>
        )}
        <label>
          How did you answer?
          <select
            disabled={busy || stale}
            value={assistance}
            onChange={(e) => {
              setAssistance(e.target.value);
              attemptToken.current = null;
            }}
          >
            <option value="unknown">Prefer not to say</option>
            <option value="independent">
              On my own, without AI, hints or the solution
            </option>
            <option value="assisted">
              With help, hints, AI or the solution
            </option>
          </select>
        </label>
        <button type="submit" disabled={busy || stale || !answer.trim()}>
          Save answer
        </button>
      </form>
      <button
        disabled={busy || stale}
        onClick={() => {
          setBusy(true);
          void learning
            .reveal(question.id)
            .then((text) => {
              setRevealed(text);
              setAssistance('assisted');
              return onSaved();
            })
            .catch(() => setError('Answer explanation unavailable.'))
            .finally(() => setBusy(false));
        }}
      >
        Reveal answer and mark future attempts assisted
      </button>
      {revealed && <pre>{revealed}</pre>}
      {error && <p role="alert">{error}</p>}
      {latest && (
        <section role="status">
          <p>
            <strong>
              {latest.feedback.grade === 'pending'
                ? 'Saved · awaiting optional advisory grading'
                : latest.feedback.grade}
            </strong>{' '}
            · {latest.assistance}
            {latest.solutionSeen ? ' · solution seen' : ''}
          </p>
          <p>{latest.feedback.text}</p>
          {latest.feedback.criteria.length > 0 && (
            <ul>
              {latest.feedback.criteria.map((c) => (
                <li key={c.criterion}>
                  {c.outcome}: {c.criterion}
                </li>
              ))}
            </ul>
          )}
          {latest.feedback.confidence !== null && (
            <p>
              Grader confidence {latest.feedback.confidence}; this is not a
              learner score.
            </p>
          )}
          {latest.feedback.grade === 'pending' && !stale && (
            <button disabled={busy} onClick={() => void onGrade(latest.id)}>
              Preview answer for advisory grading
            </button>
          )}
        </section>
      )}
      {question.attempts.length > 0 && (
        <details>
          <summary>
            Saved answer history ({question.attempts.length} most recent)
          </summary>
          <ul>
            {question.attempts.map((a) => (
              <li key={a.id}>
                <time>{new Date(a.submittedAt).toLocaleString()}</time> ·{' '}
                {a.feedback.grade} · {a.assistance}
                <pre>{a.answer}</pre>
                <p>{a.feedback.text}</p>
              </li>
            ))}
          </ul>
        </details>
      )}
      <p className="muted">
        Answers are sanitized before saving. Local questions work offline; text
        explanations remain ungraded until a separately approved request.
        Immediate retries are practice, and no knowledge promotion is inferred.
      </p>
    </section>
  );
}
