import type {
  CaptureCompleteness,
  CodingSession,
  EvidenceCompleteness,
  OverallCompleteness,
} from './session';

const UUID =
  /^[0-9a-f]{8}-[0-9a-f]{4}-[1-8][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i;
const UTC = /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d+)?Z$/;
const SESSION_STATUSES = new Set([
  'active',
  'completed',
  'interrupted',
  'incomplete',
  'failed',
]);
const EVENT_TYPES = new Set([
  'session.started',
  'user.prompt',
  'agent.message',
  'tool.started',
  'tool.completed',
  'permission.requested',
  'file.changed',
  'command.executed',
  'command.result',
  'test.result',
  'error.encountered',
  'turn.completed',
  'session.stopped',
  'context.compacted',
  'capture.gap',
]);
const TOOL_STATUSES = new Set([
  'started',
  'completed',
  'failed',
  'interrupted',
  'incomplete',
]);
const COMMAND_STATUSES = new Set([...TOOL_STATUSES, 'unknown']);
const FILE_CHANGE_KINDS = new Set([
  'added',
  'modified',
  'deleted',
  'renamed',
  'unknown',
]);
const CAPABILITY_STATES = new Set([
  'supported',
  'partial',
  'unsupported',
  'unknown',
]);
const EVIDENCE_STATES = new Set<EvidenceCompleteness>([
  'complete',
  'partial',
  'missing',
  'unknown',
  'not_applicable',
]);

export class DomainValidationError extends Error {
  constructor(message: string) {
    super(message);
    this.name = 'DomainValidationError';
  }
}

function record(value: unknown, field: string): Record<string, unknown> {
  if (typeof value !== 'object' || value === null || Array.isArray(value)) {
    throw new DomainValidationError(`${field} must be an object`);
  }
  return value as Record<string, unknown>;
}

function text(value: unknown, field: string, nullable = false): void {
  if (nullable && value === null) return;
  if (typeof value !== 'string' || value.length === 0) {
    throw new DomainValidationError(`${field} must be a non-empty string`);
  }
}

function uuid(value: unknown, field: string, nullable = false): void {
  if (nullable && value === null) return;
  if (typeof value !== 'string' || !UUID.test(value)) {
    throw new DomainValidationError(`${field} must be a UUID`);
  }
}

function utc(value: unknown, field: string, nullable = false): void {
  if (nullable && value === null) return;
  if (
    typeof value !== 'string' ||
    !UTC.test(value) ||
    Number.isNaN(Date.parse(value))
  ) {
    throw new DomainValidationError(
      `${field} must be an RFC3339 UTC timestamp`,
    );
  }
}

function array(value: unknown, field: string): readonly unknown[] {
  if (!Array.isArray(value)) {
    throw new DomainValidationError(`${field} must be an array`);
  }
  return value;
}

function enumValue(
  value: unknown,
  allowed: ReadonlySet<string>,
  field: string,
): void {
  if (typeof value !== 'string' || !allowed.has(value)) {
    throw new DomainValidationError(`${field} has an invalid value`);
  }
}

function timeRange(start: unknown, end: unknown, field: string): void {
  utc(start, `${field}.startedAt`);
  utc(end, `${field}.endedAt`, true);
  if (
    typeof start === 'string' &&
    typeof end === 'string' &&
    Date.parse(start) > Date.parse(end)
  ) {
    throw new DomainValidationError(`${field} has an invalid time range`);
  }
}

function relativePath(value: unknown, field: string): void {
  text(value, field);
  if (
    typeof value === 'string' &&
    (value.startsWith('/') ||
      /^[A-Za-z]:[\\/]/.test(value) ||
      value.split(/[\\/]/).includes('..'))
  ) {
    throw new DomainValidationError(`${field} must be approved-root-relative`);
  }
}

function validateCapabilities(value: unknown): void {
  const capabilities = record(value, 'captureCapabilities');
  for (const key of [
    'sessionLifecycle',
    'userPrompt',
    'agentResponse',
    'toolActivity',
    'commands',
    'fileActivity',
    'permissions',
    'interrupts',
    'gitContext',
  ]) {
    enumValue(
      capabilities[key],
      CAPABILITY_STATES,
      `captureCapabilities.${key}`,
    );
  }
}

function validateCompleteness(value: unknown): void {
  const completeness = record(value, 'captureCompleteness');
  for (const key of [
    'sessionLifecycle',
    'prompt',
    'agentResponse',
    'tools',
    'commands',
    'files',
    'git',
    'interrupts',
  ]) {
    enumValue(completeness[key], EVIDENCE_STATES, `captureCompleteness.${key}`);
  }
}

function validateSource(value: unknown): void {
  const source = record(value, 'source');
  text(source.provider, 'source.provider');
  enumValue(
    source.clientSurface,
    new Set(['cli', 'desktop', 'unknown']),
    'source.clientSurface',
  );
  text(source.clientVersion, 'source.clientVersion', true);
  text(source.adapterVersion, 'source.adapterVersion');
  text(source.transport, 'source.transport');
}

function validateGit(value: unknown): void {
  const context = record(value, 'gitContext');
  if (context.availability === 'unavailable') {
    enumValue(
      context.reason,
      new Set([
        'not_repository',
        'not_authorized',
        'collection_failed',
        'not_captured',
        'unknown',
      ]),
      'gitContext.reason',
    );
    return;
  }
  if (
    context.availability !== 'available' &&
    context.availability !== 'final_only'
  ) {
    throw new DomainValidationError(
      'gitContext.availability has an invalid value',
    );
  }
  const validateSnapshot = (input: unknown, name: string): void => {
    const snapshot = record(input, name);
    text(snapshot.repositoryRoot, `${name}.repositoryRoot`);
    utc(snapshot.capturedAt, `${name}.capturedAt`);
    enumValue(
      snapshot.workingTreeState,
      new Set(['clean', 'dirty', 'unknown']),
      `${name}.workingTreeState`,
    );
    for (const [index, item] of array(
      snapshot.files,
      `${name}.files`,
    ).entries()) {
      const file = record(item, `${name}.files[${index}]`);
      relativePath(file.path, `${name}.files[${index}].path`);
      if (file.previousPath !== null) {
        relativePath(file.previousPath, `${name}.files[${index}].previousPath`);
      }
      enumValue(
        file.changeType,
        FILE_CHANGE_KINDS,
        `${name}.files[${index}].changeType`,
      );
      if (file.changeType === 'renamed' && file.previousPath === null) {
        throw new DomainValidationError(
          `${name}.files[${index}] renamed without previousPath`,
        );
      }
    }
    if (typeof snapshot.truncated !== 'boolean') {
      throw new DomainValidationError(`${name}.truncated must be boolean`);
    }
  };
  if (context.availability === 'final_only') {
    enumValue(
      context.baselineReason,
      new Set([
        'not_repository',
        'not_authorized',
        'collection_failed',
        'not_captured',
        'unknown',
      ]),
      'gitContext.baselineReason',
    );
    validateSnapshot(context.after, 'gitContext.after');
    return;
  }
  validateSnapshot(context.before, 'gitContext.before');
  if (context.after !== null) {
    validateSnapshot(context.after, 'gitContext.after');
    const before = record(context.before, 'gitContext.before');
    const after = record(context.after, 'gitContext.after');
    if (before.repositoryRoot !== after.repositoryRoot) {
      throw new DomainValidationError('Git snapshots use different roots');
    }
    if (
      typeof before.capturedAt === 'string' &&
      typeof after.capturedAt === 'string' &&
      Date.parse(before.capturedAt) > Date.parse(after.capturedAt)
    ) {
      throw new DomainValidationError(
        'Git snapshots have an invalid time range',
      );
    }
  }
}

export function parseCodingSession(value: unknown): CodingSession {
  const session = record(value, 'session');
  if (session.schemaVersion !== 1) {
    throw new DomainValidationError(
      'unsupported coding session schema version',
    );
  }
  uuid(session.id, 'session.id');
  uuid(session.projectId, 'session.projectId');
  validateSource(session.source);
  timeRange(session.startedAt, session.endedAt, 'session');
  enumValue(session.status, SESSION_STATUSES, 'session.status');
  if (session.status === 'active' && session.endedAt !== null) {
    throw new DomainValidationError('active sessions cannot have endedAt');
  }
  if (
    ['completed', 'interrupted', 'failed'].includes(String(session.status)) &&
    session.endedAt === null
  ) {
    throw new DomainValidationError('terminal sessions require endedAt');
  }

  const eventIds = new Set<string>();
  const sequences = new Set<number>();
  for (const [index, item] of array(
    session.events,
    'session.events',
  ).entries()) {
    const event = record(item, `events[${index}]`);
    if (event.schemaVersion !== 1) {
      throw new DomainValidationError(
        `events[${index}] has an unsupported schema`,
      );
    }
    uuid(event.id, `events[${index}].id`);
    if (
      event.sessionId !== session.id ||
      event.projectId !== session.projectId
    ) {
      throw new DomainValidationError(
        `events[${index}] references another aggregate`,
      );
    }
    if (eventIds.has(String(event.id))) {
      throw new DomainValidationError('event IDs must be unique');
    }
    eventIds.add(String(event.id));
    if (
      !Number.isSafeInteger(event.sequence) ||
      Number(event.sequence) <= 0 ||
      sequences.has(Number(event.sequence))
    ) {
      throw new DomainValidationError(
        'event sequences must be positive and unique',
      );
    }
    sequences.add(Number(event.sequence));
    utc(event.receivedAt, `events[${index}].receivedAt`);
    enumValue(event.eventType, EVENT_TYPES, `events[${index}].eventType`);
    record(event.payload, `events[${index}].payload`);
  }

  const turnIds = new Set<string>();
  for (const [index, item] of array(session.turns, 'session.turns').entries()) {
    const turn = record(item, `turns[${index}]`);
    uuid(turn.id, `turns[${index}].id`);
    if (turn.sessionId !== session.id || turnIds.has(String(turn.id))) {
      throw new DomainValidationError(
        `turns[${index}] has an invalid aggregate reference`,
      );
    }
    turnIds.add(String(turn.id));
    utc(turn.startedAt, `turns[${index}].startedAt`, true);
    utc(turn.endedAt, `turns[${index}].endedAt`, true);
    enumValue(turn.status, SESSION_STATUSES, `turns[${index}].status`);
    text(turn.sourceTurnId, `turns[${index}].sourceTurnId`, true);
    text(turn.userPrompt, `turns[${index}].userPrompt`, true);
    for (const id of array(turn.eventIds, `turns[${index}].eventIds`)) {
      if (!eventIds.has(String(id))) {
        throw new DomainValidationError(
          `turns[${index}] references an unknown event`,
        );
      }
    }
  }

  for (const [index, item] of array(
    session.events,
    'session.events',
  ).entries()) {
    const event = record(item, `events[${index}]`);
    if (event.turnId !== null && !turnIds.has(String(event.turnId))) {
      throw new DomainValidationError(
        `events[${index}] references an unknown turn`,
      );
    }
  }

  const toolIds = new Set<string>();
  for (const [index, item] of array(
    session.toolExecutions,
    'session.toolExecutions',
  ).entries()) {
    const tool = record(item, `toolExecutions[${index}]`);
    uuid(tool.id, `toolExecutions[${index}].id`);
    if (toolIds.has(String(tool.id)) || tool.sessionId !== session.id) {
      throw new DomainValidationError(
        `toolExecutions[${index}] has an invalid aggregate reference`,
      );
    }
    toolIds.add(String(tool.id));
    if (tool.turnId !== null && !turnIds.has(String(tool.turnId))) {
      throw new DomainValidationError(
        `toolExecutions[${index}] references an unknown turn`,
      );
    }
    text(tool.toolName, `toolExecutions[${index}].toolName`);
    utc(tool.startedAt, `toolExecutions[${index}].startedAt`, true);
    utc(tool.completedAt, `toolExecutions[${index}].completedAt`, true);
    enumValue(tool.status, TOOL_STATUSES, `toolExecutions[${index}].status`);
    if (
      ['completed', 'failed'].includes(String(tool.status)) &&
      tool.completedAt === null
    ) {
      throw new DomainValidationError(
        `toolExecutions[${index}] requires completedAt`,
      );
    }
    for (const [pathIndex, path] of array(
      tool.relatedFilePaths,
      `toolExecutions[${index}].relatedFilePaths`,
    ).entries()) {
      relativePath(
        path,
        `toolExecutions[${index}].relatedFilePaths[${pathIndex}]`,
      );
    }
  }

  const commandIds = new Set<string>();
  for (const [index, item] of array(
    session.commandExecutions,
    'session.commandExecutions',
  ).entries()) {
    const command = record(item, `commandExecutions[${index}]`);
    uuid(command.id, `commandExecutions[${index}].id`);
    if (
      commandIds.has(String(command.id)) ||
      command.sessionId !== session.id
    ) {
      throw new DomainValidationError(
        `commandExecutions[${index}] has an invalid aggregate reference`,
      );
    }
    commandIds.add(String(command.id));
    if (command.turnId !== null && !turnIds.has(String(command.turnId))) {
      throw new DomainValidationError(
        `commandExecutions[${index}] references an unknown turn`,
      );
    }
    if (
      command.toolExecutionId !== null &&
      !toolIds.has(String(command.toolExecutionId))
    ) {
      throw new DomainValidationError(
        `commandExecutions[${index}] references an unknown tool execution`,
      );
    }
    text(command.command, `commandExecutions[${index}].command`);
    utc(command.startedAt, `commandExecutions[${index}].startedAt`, true);
    utc(command.completedAt, `commandExecutions[${index}].completedAt`, true);
    enumValue(
      command.status,
      COMMAND_STATUSES,
      `commandExecutions[${index}].status`,
    );
    if (
      ['completed', 'failed'].includes(String(command.status)) &&
      command.completedAt === null
    ) {
      throw new DomainValidationError(
        `commandExecutions[${index}] requires completedAt`,
      );
    }
    if (command.exitCode !== null && !Number.isSafeInteger(command.exitCode)) {
      throw new DomainValidationError(
        `commandExecutions[${index}].exitCode must be an integer or null`,
      );
    }
  }

  const fileChangeIds = new Set<string>();
  for (const [index, item] of array(
    session.fileChanges,
    'session.fileChanges',
  ).entries()) {
    const change = record(item, `fileChanges[${index}]`);
    uuid(change.id, `fileChanges[${index}].id`);
    if (fileChangeIds.has(String(change.id))) {
      throw new DomainValidationError('file change IDs must be unique');
    }
    fileChangeIds.add(String(change.id));
    relativePath(change.path, `fileChanges[${index}].path`);
    if (change.previousPath !== null) {
      relativePath(change.previousPath, `fileChanges[${index}].previousPath`);
    }
    enumValue(
      change.changeType,
      FILE_CHANGE_KINDS,
      `fileChanges[${index}].changeType`,
    );
    if (change.changeType === 'renamed' && change.previousPath === null) {
      throw new DomainValidationError(
        `fileChanges[${index}] renamed without previousPath`,
      );
    }
  }
  validateGit(session.gitContext);
  validateCapabilities(session.captureCapabilities);
  validateCompleteness(session.captureCompleteness);
  return value as CodingSession;
}

export function deriveOverallCompleteness(
  completeness: CaptureCompleteness,
): OverallCompleteness {
  const dimensions = Object.values(completeness);
  if (dimensions.includes('missing')) return 'degraded';
  if (
    dimensions.every(
      (value) => value === 'complete' || value === 'not_applicable',
    )
  ) {
    return 'complete';
  }
  return 'partial';
}
