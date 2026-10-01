import { invoke } from '@tauri-apps/api/core';
export interface ProjectView {
  id: string;
  name: string;
  root: string;
  tracking: boolean;
  policyRevision: number;
}
export interface SessionView {
  id: string;
  startedAt: string;
  endedAt: string | null;
  captureState: string;
  coverage: string;
  revision: number;
  paused: boolean;
  restarted: boolean;
  lateEvidence: boolean;
  continuationOf: string | null;
}
export interface ListCursor {
  startedAt: string;
  sessionId: string;
}
export interface SessionList {
  items: SessionView[];
  next: ListCursor | null;
}
export interface EventView {
  id: string;
  sequence: number;
  title: string;
  text: string;
  truncated: boolean;
}
export interface CodeView {
  path: string;
  content: string | null;
  omitted: boolean;
}
export interface SessionDetail {
  session: SessionView;
  events: EventView[];
  nextSequence: number | null;
  eventCount: number;
  code: CodeView[];
  gitNotice: string;
}
export interface InstallPreview {
  schemaVersion: 1;
  planId: string;
  action: 'install' | 'upgrade' | 'disconnect' | 'rollback';
  target: string;
  ownedCommand: string;
  events: string[];
  changesConfiguration: boolean;
  requiresCodexTrustReview: boolean;
  backupRetentionDays: number;
}
export interface LocalStatus {
  spoolEvictionCount: number;
  schemaVersion: 1;
  message: string;
  remoteAnalysisEnabled: boolean;
}
const failure =
  'The action could not be completed safely. Check the project permissions, Codex CLI version and existing hooks, then retry.';
function object(v: unknown): Record<string, unknown> {
  if (typeof v !== 'object' || v === null || Array.isArray(v))
    throw new Error(failure);
  return v as Record<string, unknown>;
}
function text(v: unknown, max = 4096): string {
  if (typeof v !== 'string' || v.length > max) throw new Error(failure);
  return v;
}
function flag(v: unknown): boolean {
  if (typeof v !== 'boolean') throw new Error(failure);
  return v;
}
function number(v: unknown, max = Number.MAX_SAFE_INTEGER): number {
  if (typeof v !== 'number' || !Number.isSafeInteger(v) || v < 0 || v > max)
    throw new Error(failure);
  return v;
}
function id(v: unknown): string {
  const s = text(v, 36);
  if (
    !/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i.test(s)
  )
    throw new Error(failure);
  return s;
}
function nullable<T>(v: unknown, parse: (v: unknown) => T): T | null {
  return v === null ? null : parse(v);
}
function date(v: unknown): string {
  const s = text(v, 64);
  if (!Number.isFinite(Date.parse(s))) throw new Error(failure);
  return s;
}
function array<T>(v: unknown, max: number, parse: (v: unknown) => T): T[] {
  if (!Array.isArray(v) || v.length > max) throw new Error(failure);
  return v.map(parse);
}
export function parseProject(v: unknown): ProjectView {
  const o = object(v);
  return {
    id: id(o.id),
    name: text(o.name, 80),
    root: text(o.root),
    tracking: flag(o.tracking),
    policyRevision: number(o.policyRevision),
  };
}
export function parseSession(v: unknown): SessionView {
  const o = object(v);
  const state = text(o.captureState, 32);
  if (
    ![
      'active',
      'idle',
      'interrupted',
      'finalized',
      'deleted',
      'completed',
      'incomplete',
      'failed',
    ].includes(state)
  )
    throw new Error(failure);
  const coverage = text(o.coverage, 32);
  if (!['complete', 'partial', 'degraded'].includes(coverage))
    throw new Error(failure);
  return {
    id: id(o.id),
    startedAt: date(o.startedAt),
    endedAt: nullable(o.endedAt, date),
    captureState: state,
    coverage,
    revision: number(o.revision),
    paused: flag(o.paused),
    restarted: flag(o.restarted),
    lateEvidence: flag(o.lateEvidence),
    continuationOf: nullable(o.continuationOf, id),
  };
}
function cursor(v: unknown): ListCursor {
  const o = object(v);
  return { startedAt: date(o.startedAt), sessionId: id(o.sessionId) };
}
export function parseDetail(v: unknown): SessionDetail {
  const o = object(v);
  return {
    session: parseSession(o.session),
    events: array(o.events, 50, (v) => {
      const e = object(v);
      return {
        id: id(e.id),
        sequence: number(e.sequence),
        title: text(e.title, 80),
        text: text(e.text, 32768),
        truncated: flag(e.truncated),
      };
    }),
    nextSequence: nullable(o.nextSequence, number),
    eventCount: number(o.eventCount, 20000),
    code: array(o.code, 100, (v) => {
      const f = object(v);
      const path = text(f.path);
      if (path.startsWith('/') || path.split('/').includes('..'))
        throw new Error(failure);
      return {
        path,
        content: nullable(f.content, (v) => text(v, 32768)),
        omitted: flag(f.omitted),
      };
    }),
    gitNotice: text(o.gitNotice),
  };
}
export function parsePreview(v: unknown): InstallPreview {
  const o = object(v);
  if (
    o.schemaVersion !== 1 ||
    !['install', 'upgrade', 'disconnect', 'rollback'].includes(text(o.action))
  )
    throw new Error(failure);
  return {
    schemaVersion: 1,
    planId: id(o.planId),
    action: o.action as InstallPreview['action'],
    target: text(o.target),
    ownedCommand: text(o.ownedCommand, 16384),
    events: array(o.events, 12, (v) => text(v, 80)),
    changesConfiguration: flag(o.changesConfiguration),
    requiresCodexTrustReview: flag(o.requiresCodexTrustReview),
    backupRetentionDays: number(o.backupRetentionDays, 7),
  };
}
async function call<T>(
  command: string,
  args: Record<string, unknown>,
  parse: (v: unknown) => T,
): Promise<T> {
  try {
    return parse(await invoke<unknown>(command, args));
  } catch {
    throw new Error(failure);
  }
}
const empty = (v: unknown): void => {
  if (v !== null && v !== undefined) throw new Error(failure);
};
export const localCapture = {
  status: () =>
    call('local_status', {}, (v) => {
      const o = object(v);
      if (o.schemaVersion !== 1) throw new Error(failure);
      return {
        schemaVersion: 1 as const,
        message: text(o.message),
        remoteAnalysisEnabled: flag(o.remoteAnalysisEnabled),
        spoolEvictionCount: number(
          o.spoolEvictionCount,
          Number.MAX_SAFE_INTEGER,
        ),
      };
    }),
  projects: (after: string | null = null) =>
    call('list_projects', { after }, (v) => array(v, 50, parseProject)),
  approve: (path: string, name: string) =>
    call('approve_project', { path, name }, parseProject),
  preview: (projectId: string, disconnect = false) =>
    call('preview_connection', { projectId, disconnect }, parsePreview),
  apply: (planId: string, enableLocalCapture: boolean) =>
    call('apply_connection', { planId, enableLocalCapture }, empty),
  tracking: (projectId: string, enabled: boolean) =>
    call('set_tracking', { projectId, enabled }, empty),
  pauseAll: () => call('pause_all_capture', {}, empty),
  sessions: (projectId: string, before: ListCursor | null = null) =>
    call('list_sessions', { projectId, before }, (v) => {
      const o = object(v);
      return {
        items: array(o.items, 50, parseSession),
        next: nullable(o.next, cursor),
      };
    }),
  detail: (sessionId: string, afterSequence: number | null = null) =>
    call('session_detail', { sessionId, afterSequence }, parseDetail),
  finish: (sessionId: string) => call('finish_session', { sessionId }, empty),
  deleteSession: (sessionId: string) =>
    call('delete_session', { sessionId, confirmed: true }, empty),
  deleteProject: (projectId: string) =>
    call('delete_project', { projectId, confirmed: true }, empty),
};
