import completeSessionJson from './coding-session-cli-complete.json';
import { parseCodingSession } from '../src/validation';

export const CLI_COMPLETE_SESSION = parseCodingSession(completeSessionJson);

export const CLI_INTERRUPTED_SESSION = parseCodingSession({
  ...CLI_COMPLETE_SESSION,
  status: 'interrupted',
  commandExecutions: CLI_COMPLETE_SESSION.commandExecutions.map((command) => ({
    ...command,
    completedAt: null,
    status: 'interrupted',
    exitCode: null,
    outputSummary: null,
  })),
  captureCompleteness: {
    ...CLI_COMPLETE_SESSION.captureCompleteness,
    commands: 'partial',
    interrupts: 'complete',
  },
});

export const DESKTOP_PARTIAL_PROMPT_SESSION = parseCodingSession({
  ...CLI_COMPLETE_SESSION,
  source: {
    ...CLI_COMPLETE_SESSION.source,
    clientSurface: 'desktop',
    clientVersion: '26.908.40834',
  },
  endedAt: null,
  status: 'incomplete',
  turns: CLI_COMPLETE_SESSION.turns.map((turn) => ({
    ...turn,
    sourceTurnId: null,
    userPrompt: null,
    eventIds: [],
  })),
  events: [],
  captureCapabilities: {
    ...CLI_COMPLETE_SESSION.captureCapabilities,
    userPrompt: 'unknown',
    interrupts: 'unknown',
  },
  captureCompleteness: {
    ...CLI_COMPLETE_SESSION.captureCompleteness,
    sessionLifecycle: 'partial',
    prompt: 'unknown',
    interrupts: 'unknown',
  },
});

export const DIRTY_GIT_SESSION = CLI_COMPLETE_SESSION;

export const NO_GIT_SESSION = parseCodingSession({
  ...CLI_COMPLETE_SESSION,
  gitContext: {
    availability: 'unavailable',
    reason: 'not_repository',
  },
  captureCapabilities: {
    ...CLI_COMPLETE_SESSION.captureCapabilities,
    gitContext: 'unsupported',
  },
  captureCompleteness: {
    ...CLI_COMPLETE_SESSION.captureCompleteness,
    git: 'not_applicable',
  },
});
