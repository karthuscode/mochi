import { describe, expect, it } from 'vitest';
import {
  CLI_COMPLETE_SESSION,
  CLI_INTERRUPTED_SESSION,
  DESKTOP_PARTIAL_PROMPT_SESSION,
  DIRTY_GIT_SESSION,
  NO_GIT_SESSION,
} from '../fixtures/session-scenarios';
import {
  deriveOverallCompleteness,
  DomainValidationError,
  parseCodingSession,
} from './validation';

describe('coding session domain contract', () => {
  it('accepts the complete CLI fixture and preserves ordering metadata', () => {
    expect(CLI_COMPLETE_SESSION.status).toBe('completed');
    expect(CLI_COMPLETE_SESSION.events[0]?.sequence).toBe(1);
    expect(CLI_COMPLETE_SESSION.turns[0]?.userPrompt).toBe(
      'Add a bounded parser.',
    );
    expect(
      deriveOverallCompleteness(CLI_COMPLETE_SESSION.captureCompleteness),
    ).toBe('complete');
  });

  it('represents interrupted commands without inventing an exit code', () => {
    expect(CLI_INTERRUPTED_SESSION.status).toBe('interrupted');
    expect(CLI_INTERRUPTED_SESSION.commandExecutions[0]?.status).toBe(
      'interrupted',
    );
    expect(CLI_INTERRUPTED_SESSION.commandExecutions[0]?.exitCode).toBeNull();
  });

  it('represents Desktop prompt and interrupt uncertainty independently', () => {
    expect(DESKTOP_PARTIAL_PROMPT_SESSION.captureCapabilities.userPrompt).toBe(
      'unknown',
    );
    expect(DESKTOP_PARTIAL_PROMPT_SESSION.captureCapabilities.interrupts).toBe(
      'unknown',
    );
    expect(DESKTOP_PARTIAL_PROMPT_SESSION.turns[0]?.userPrompt).toBeNull();
    expect(
      deriveOverallCompleteness(
        DESKTOP_PARTIAL_PROMPT_SESSION.captureCompleteness,
      ),
    ).toBe('partial');
  });

  it('preserves pre-existing dirty Git state in before and after snapshots', () => {
    expect(DIRTY_GIT_SESSION.gitContext.availability).toBe('available');
    if (DIRTY_GIT_SESSION.gitContext.availability === 'available') {
      expect(DIRTY_GIT_SESSION.gitContext.before.files).toHaveLength(1);
      expect(DIRTY_GIT_SESSION.gitContext.after?.files).toHaveLength(2);
      expect(DIRTY_GIT_SESSION.gitContext.before.files[0]?.path).toBe(
        'src/preexisting.ts',
      );
    }
  });

  it('supports sessions where Git is unavailable', () => {
    expect(NO_GIT_SESSION.gitContext).toEqual({
      availability: 'unavailable',
      reason: 'not_repository',
    });
    expect(NO_GIT_SESSION.captureCompleteness.git).toBe('not_applicable');
  });

  it('rejects reversed session timestamps and unsupported versions', () => {
    expect(() =>
      parseCodingSession({
        ...CLI_COMPLETE_SESSION,
        endedAt: '2026-09-17T09:00:00Z',
      }),
    ).toThrow(DomainValidationError);
    expect(() =>
      parseCodingSession({ ...CLI_COMPLETE_SESSION, schemaVersion: 2 }),
    ).toThrow('unsupported coding session schema version');
  });

  it('rejects duplicate event sequences without sorting the input', () => {
    const duplicate = {
      ...CLI_COMPLETE_SESSION.events[0],
      id: '40000000-0000-4000-8000-000000000002',
    };
    expect(() =>
      parseCodingSession({
        ...CLI_COMPLETE_SESSION,
        events: [...CLI_COMPLETE_SESSION.events, duplicate],
      }),
    ).toThrow('event sequences must be positive and unique');
  });
});

it('preserves final-only Git context and rejects an invented baseline reason', () => {
  const data = structuredClone(CLI_COMPLETE_SESSION);
  if (data.gitContext.availability !== 'available') throw new Error('fixture');
  const final = {
    ...data,
    gitContext: {
      availability: 'final_only',
      after: data.gitContext.after ?? data.gitContext.before,
      baselineReason: 'not_captured',
    },
  };
  expect(parseCodingSession(final).gitContext.availability).toBe('final_only');
  expect(() =>
    parseCodingSession({
      ...final,
      gitContext: { ...final.gitContext, baselineReason: 'made_up' },
    }),
  ).toThrow(DomainValidationError);
});
