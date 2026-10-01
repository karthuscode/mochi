import { invoke } from '@tauri-apps/api/core';
export interface AnalysisStatus {
  enabled: boolean;
  keyConfigured: boolean;
  running: boolean;
  message: string;
  model: string;
}
export interface SendPreview {
  token: string;
  sessionId: string;
  inputRevision: number;
  inputHash: string;
  requestHash: string;
  provider: string;
  model: string;
  endpoint: string;
  purpose: 'generation' | 'grading';
  requestJson: string;
  expiresInSeconds: number;
}
export type Grade = 'correct' | 'incorrect' | 'uncertain' | 'pending';
export interface Attempt {
  id: string;
  questionId: string;
  answer: string;
  assistance: 'independent' | 'assisted' | 'unknown';
  solutionSeen: boolean;
  feedback: {
    criteria: { criterion: string; outcome: 'pass' | 'fail' | 'uncertain' }[];
    blockingMisconception: boolean | null;
    grade: Grade;
    method: string;
    confidence: number | null;
    text: string;
  };
  submittedAt: string;
}
export interface Question {
  id: string;
  kind: 'mcq' | 'predict_output' | 'explain_why';
  prompt: string;
  choices: { id: string; text: string }[];
  snippet: string | null;
  assumptions: string | null;
  revealed: boolean;
  attempts: Attempt[];
}
export interface Claim {
  text: string;
  confidence: 'observed' | 'inferred' | 'unknown';
  references: string[];
}
export interface Evidence {
  id: string;
  kind: string;
  text: string;
  path: string | null;
  capturedAt: string;
  firstLine: number | null;
  excerptHash: string;
  truncated: boolean;
  verifiedSuccess: boolean;
}
export interface Card {
  key: string;
  title: string;
  definition: string;
  whyItWorks: string;
  whyItMatters: string;
  misconception: string;
  references: string[];
  codeReference: string | null;
  codeQuote: string | null;
  components: {
    novelty: number;
    relevance: number;
    importance: number;
    impact: number;
    weakness: number;
  };
  selectionScore: number;
  question: Question;
}
export interface Lesson {
  id: string;
  sessionId: string;
  inputRevision: number;
  inputHash: string;
  model: string;
  stale: boolean;
  status: 'internal_explanation' | 'insufficient_context';
  coverage: string;
  omissions: string[];
  evidence: Evidence[];
  reconstruction: Record<
    'overview' | 'built' | 'changed' | 'decisions' | 'failures' | 'unresolved',
    Claim[]
  >;
  cards: Card[];
}
const failure =
  'The learning action could not be completed safely. Check the current session and remote permissions, then retry.';
function obj(v: unknown): Record<string, unknown> {
  if (typeof v !== 'object' || v === null || Array.isArray(v))
    throw Error(failure);
  return v as Record<string, unknown>;
}
function str(v: unknown, max = 4096): string {
  if (typeof v !== 'string' || v.length > max) throw Error(failure);
  return v;
}
function bool(v: unknown): boolean {
  if (typeof v !== 'boolean') throw Error(failure);
  return v;
}
function num(v: unknown, max = Number.MAX_SAFE_INTEGER): number {
  if (typeof v !== 'number' || !Number.isFinite(v) || v < 0 || v > max)
    throw Error(failure);
  return v;
}
function integer(v: unknown): number {
  const n = num(v);
  if (!Number.isSafeInteger(n)) throw Error(failure);
  return n;
}
function uuid(v: unknown): string {
  const s = str(v, 36);
  if (!/^[0-9a-f]{8}(-[0-9a-f]{4}){3}-[0-9a-f]{12}$/i.test(s))
    throw Error(failure);
  return s;
}
function hash(v: unknown): string {
  const s = str(v, 64);
  if (!/^[0-9a-f]{64}$/.test(s)) throw Error(failure);
  return s;
}
function optional<T>(v: unknown, parse: (v: unknown) => T): T | null {
  return v === null ? null : parse(v);
}
function arr<T>(v: unknown, max: number, parse: (v: unknown) => T): T[] {
  if (!Array.isArray(v) || v.length > max) throw Error(failure);
  return v.map(parse);
}
function one<T extends string>(v: unknown, allowed: readonly T[]): T {
  const s = str(v);
  if (!allowed.includes(s as T)) throw Error(failure);
  return s as T;
}
export function parseAnalysisStatus(v: unknown): AnalysisStatus {
  const o = obj(v);
  return {
    enabled: bool(o.enabled),
    keyConfigured: bool(o.keyConfigured),
    running: bool(o.running),
    message: str(o.message),
    model: str(o.model, 80),
  };
}
export function parseSendPreview(v: unknown): SendPreview {
  const o = obj(v);
  if (
    o.provider !== 'OpenAI' ||
    o.endpoint !== 'https://api.openai.com/v1/responses'
  )
    throw Error(failure);
  const requestJson = str(o.requestJson, 128 * 1024);
  obj(JSON.parse(requestJson) as unknown);
  return {
    token: uuid(o.token),
    sessionId: uuid(o.sessionId),
    inputRevision: integer(o.inputRevision),
    inputHash: hash(o.inputHash),
    requestHash: hash(o.requestHash),
    provider: 'OpenAI',
    model: str(o.model, 80),
    endpoint: str(o.endpoint),
    purpose: one(o.purpose, ['generation', 'grading']),
    requestJson,
    expiresInSeconds: integer(o.expiresInSeconds),
  };
}
export function parseAttempt(v: unknown): Attempt {
  const o = obj(v),
    f = obj(o.feedback);
  return {
    id: uuid(o.id),
    questionId: uuid(o.questionId),
    answer: str(o.answer, 16 * 1024),
    assistance: one(o.assistance, ['independent', 'assisted', 'unknown']),
    solutionSeen: bool(o.solutionSeen),
    feedback: {
      criteria: arr(f.criteria, 4, (v) => {
        const c = obj(v);
        return {
          criterion: str(c.criterion, 400),
          outcome: one(c.outcome, ['pass', 'fail', 'uncertain']),
        };
      }),
      blockingMisconception: optional(f.blockingMisconception, bool),
      grade: one(f.grade, ['correct', 'incorrect', 'uncertain', 'pending']),
      method: one(f.method, ['local', 'ai_advisory', 'ungraded']),
      confidence: optional(f.confidence, (v) => num(v, 1)),
      text: str(f.text, 4096),
    },
    submittedAt: str(o.submittedAt, 64),
  };
}
function question(v: unknown): Question {
  const o = obj(v);
  return {
    id: uuid(o.id),
    kind: one(o.kind, ['mcq', 'predict_output', 'explain_why']),
    prompt: str(o.prompt, 1500),
    choices: arr(o.choices, 4, (v) => {
      const c = obj(v);
      return { id: str(c.id, 16), text: str(c.text, 500) };
    }),
    snippet: optional(o.snippet, (v) => str(v, 1000)),
    assumptions: optional(o.assumptions, (v) => str(v, 400)),
    revealed: bool(o.revealed),
    attempts: arr(o.attempts, 50, parseAttempt),
  };
}
export function parseLesson(v: unknown): Lesson {
  const o = obj(v),
    r = obj(o.reconstruction);
  const claim = (v: unknown): Claim => {
    const c = obj(v);
    return {
      text: str(c.text, 1800),
      confidence: one(c.confidence, ['observed', 'inferred', 'unknown']),
      references: arr(c.references, 8, (v) => str(v, 100)),
    };
  };
  return {
    id: uuid(o.id),
    sessionId: uuid(o.sessionId),
    inputRevision: integer(o.inputRevision),
    inputHash: hash(o.inputHash),
    model: str(o.model, 80),
    stale: bool(o.stale),
    status: one(o.status, ['internal_explanation', 'insufficient_context']),
    coverage: str(o.coverage, 32),
    omissions: arr(o.omissions, 20, (v) => str(v, 500)),
    evidence: arr(o.evidence, 70, (v) => {
      const e = obj(v),
        path = optional(e.path, str);
      if (path?.startsWith('/') || path?.split('/').includes('..'))
        throw Error(failure);
      return {
        id: str(e.id, 100),
        kind: str(e.kind, 32),
        text: str(e.text, 3000),
        path,
        capturedAt: str(e.capturedAt, 64),
        firstLine: optional(e.firstLine, integer),
        excerptHash: hash(e.excerptHash),
        truncated: bool(e.truncated),
        verifiedSuccess: bool(e.verifiedSuccess),
      };
    }),
    reconstruction: {
      overview: arr(r.overview, 8, claim),
      built: arr(r.built, 8, claim),
      changed: arr(r.changed, 8, claim),
      decisions: arr(r.decisions, 8, claim),
      failures: arr(r.failures, 8, claim),
      unresolved: arr(r.unresolved, 8, claim),
    },
    cards: arr(o.cards, 3, (v) => {
      const c = obj(v),
        n = obj(c.components);
      return {
        key: str(c.key, 80),
        title: str(c.title, 100),
        definition: str(c.definition, 1800),
        whyItWorks: str(c.whyItWorks, 3000),
        whyItMatters: str(c.whyItMatters, 2000),
        misconception: str(c.misconception, 1000),
        references: arr(c.references, 8, (v) => str(v, 100)),
        codeReference: optional(c.codeReference, str),
        codeQuote: optional(c.codeQuote, (v) => str(v, 2200)),
        components: {
          novelty: num(n.novelty, 1),
          relevance: num(n.relevance, 1),
          importance: num(n.importance, 1),
          impact: num(n.impact, 1),
          weakness: num(n.weakness, 1),
        },
        selectionScore: num(c.selectionScore, 1),
        question: question(c.question),
      };
    }),
  };
}
const safeErrors = new Set([
  'Insufficient technical evidence for a useful explanation. A prompt alone is not enough.',
  'Finish the observed session before preparing analysis.',
  'Current exclusions could not be checked. No analysis is prepared.',
  'Enable remote analysis first. Key presence is not permission.',
  'Store your API key in Keychain before sending.',
  'Preview expired or permissions changed. Review it again.',
  'Session evidence or exclusions changed. Preview again.',
  'Use a valid OpenAI API key.',
  'Keychain is unavailable or access was denied.',
  'An analysis job is already running.',
]);
async function call<T>(
  command: string,
  args: Record<string, unknown>,
  parse: (v: unknown) => T,
): Promise<T> {
  let message = failure;
  try {
    return parse(await invoke<unknown>(command, args));
  } catch (error) {
    // This IPC boundary keeps only whitelisted public errors, never native diagnostics.
    if (typeof error === 'string' && safeErrors.has(error)) message = error;
  }
  throw new Error(message);
}

const empty = (v: unknown): void => {
  if (v !== null && v !== undefined) throw Error(failure);
};
export const learning = {
  status: () => call('analysis_status', {}, parseAnalysisStatus),
  permission: (enabled: boolean) =>
    call('analysis_permission', { enabled }, empty),
  setKey: (key: string) => call('store_api_key', { key }, empty),
  deleteKey: () => call('delete_api_key', {}, empty),
  cancel: () => call('cancel_analysis', {}, empty),
  preview: (sessionId: string, attemptId: string | null = null) =>
    call('preview_analysis', { sessionId, attemptId }, parseSendPreview),
  send: (token: string) =>
    call('approve_analysis', { token, confirmed: true }, empty),
  lesson: (sessionId: string) =>
    call('learning_document', { sessionId }, (v) => optional(v, parseLesson)),
  submit: (
    attemptId: string,
    questionId: string,
    answer: string,
    assistance: string,
  ) =>
    call(
      'submit_selfcheck',
      { attemptId, questionId, answer, assistance },
      parseAttempt,
    ),
  reveal: (questionId: string) =>
    call('reveal_selfcheck', { questionId }, (v) => str(v, 4000)),
};
