import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { LearningPanel, LearningSettings } from './Learning';
import { learning } from './native/learning';
import type { Lesson, SendPreview, Attempt } from './native/learning';
vi.mock('./native/learning', () => ({
  learning: {
    status: vi.fn(),
    permission: vi.fn(),
    setKey: vi.fn(),
    deleteKey: vi.fn(),
    cancel: vi.fn(),
    preview: vi.fn(),
    send: vi.fn(),
    lesson: vi.fn(),
    submit: vi.fn(),
    reveal: vi.fn(),
  },
}));
const id = '10000000-0000-4000-8000-000000000001';
const questionId = '10000000-0000-4000-8000-000000000002';
const claim = {
  text: 'The observed loop accumulates values.',
  confidence: 'observed' as const,
  references: ['code:after:fixture'],
};
const lesson: Lesson = {
  id,
  sessionId: id,
  inputRevision: 2,
  inputHash: 'a'.repeat(64),
  model: 'gpt-4o-mini-2024-07-18',
  stale: false,
  status: 'internal_explanation',
  coverage: 'partial',
  omissions: ['No reliable test result observed.'],
  evidence: [
    {
      id: 'code:after:fixture',
      kind: 'code',
      text: 'result += value',
      path: 'calculator.py',
      capturedAt: '2026-10-01T12:00:00Z',
      firstLine: 1,
      excerptHash: 'b'.repeat(64),
      truncated: false,
      verifiedSuccess: false,
    },
  ],
  reconstruction: {
    overview: [claim],
    built: [claim],
    changed: [claim],
    decisions: [claim],
    failures: [claim],
    unresolved: [claim],
  },
  cards: [
    {
      key: 'algorithms.accumulation',
      title: 'Accumulating values',
      definition: 'An accumulator preserves earlier contributions.',
      whyItWorks: 'The update combines each next input.',
      whyItMatters: 'This makes empty input intentional.',
      misconception: 'Initialization inside the loop loses history.',
      references: ['code:after:fixture'],
      codeReference: 'code:after:fixture',
      codeQuote: 'result += value',
      components: {
        novelty: 1,
        relevance: 0.9,
        importance: 0.9,
        impact: 0.9,
        weakness: 0.8,
      },
      selectionScore: 0.905,
      question: {
        id: questionId,
        kind: 'mcq',
        prompt: 'Which initial value preserves addition?',
        choices: [
          { id: 'a', text: 'Zero' },
          { id: 'b', text: 'One' },
          { id: 'c', text: 'Negative one' },
        ],
        snippet: null,
        assumptions: null,
        revealed: false,
        attempts: [],
      },
    },
  ],
};
const preview: SendPreview = {
  token: id,
  sessionId: id,
  inputRevision: 2,
  inputHash: 'a'.repeat(64),
  requestHash: 'c'.repeat(64),
  provider: 'OpenAI',
  model: 'gpt-4o-mini-2024-07-18',
  endpoint: 'https://api.openai.com/v1/responses',
  purpose: 'generation',
  requestJson: JSON.stringify({
    model: 'gpt-4o-mini-2024-07-18',
    store: false,
    input: 'Synthetic evidence only',
  }),
  expiresInSeconds: 300,
};
beforeEach(() => {
  vi.resetAllMocks();
  vi.mocked(learning.lesson).mockResolvedValue(lesson);
  vi.mocked(learning.preview).mockResolvedValue(preview);
  vi.mocked(learning.status).mockResolvedValue({
    enabled: false,
    keyConfigured: false,
    running: false,
    message: 'Remote analysis is off.',
    model: 'gpt-4o-mini-2024-07-18',
  });
  vi.mocked(learning.permission).mockResolvedValue();
  vi.mocked(learning.send).mockResolvedValue();
});
describe('internal learning controls', () => {
  it('never sends on mount or preview, and requires the exact send checkbox', async () => {
    render(<LearningPanel sessionId={id} finalized />);
    await screen.findByRole('heading', { name: 'Learn from this work' });
    expect(learning.send).not.toHaveBeenCalled();
    fireEvent.click(
      screen.getByRole('button', { name: 'Preview explanation request' }),
    );
    await screen.findByRole('heading', {
      name: 'Review exactly what will be sent',
    });
    const send = screen.getByRole('button', { name: 'Send approved request' });
    expect(send).toBeDisabled();
    expect(learning.send).not.toHaveBeenCalled();
    fireEvent.click(
      screen.getByRole('checkbox', {
        name: /I approve sending this exact session request/,
      }),
    );
    expect(send).toBeEnabled();
    fireEvent.click(send);
    await waitFor(() =>
      expect(learning.send).toHaveBeenCalledExactlyOnceWith(id),
    );
  });
  it('does not expose the answer explanation before submission and saves one attempt', async () => {
    const answer: Attempt = {
      id: '10000000-0000-4000-8000-000000000003',
      questionId,
      answer: 'b',
      assistance: 'unknown',
      solutionSeen: false,
      feedback: {
        grade: 'incorrect',
        method: 'local',
        confidence: null,
        criteria: [],
        blockingMisconception: null,
        text: 'Zero is the additive identity.',
      },
      submittedAt: '2026-10-01T12:00:00Z',
    };
    vi.mocked(learning.submit).mockResolvedValue(answer);
    render(<LearningPanel sessionId={id} finalized />);
    await screen.findByText('Which initial value preserves addition?');
    expect(
      screen.queryByText('Zero is the additive identity.'),
    ).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole('radio', { name: 'One' }));
    fireEvent.click(screen.getByRole('button', { name: 'Save answer' }));
    expect(
      await screen.findByText('Zero is the additive identity.'),
    ).toBeVisible();
    expect(learning.submit).toHaveBeenCalledTimes(1);
    expect(learning.submit).toHaveBeenCalledWith(
      expect.any(String),
      questionId,
      'b',
      'unknown',
    );
  });
  it('marks a stale explanation and disables submitting its questions', async () => {
    vi.mocked(learning.lesson).mockResolvedValue({ ...lesson, stale: true });
    render(<LearningPanel sessionId={id} finalized />);
    expect(await screen.findByRole('alert')).toHaveTextContent(
      'earlier explanation',
    );
    expect(screen.getByRole('button', { name: 'Save answer' })).toBeDisabled();
    expect(learning.send).not.toHaveBeenCalled();
  });
  it('requires finalization before a session request can be prepared', () => {
    render(<LearningPanel sessionId={id} finalized={false} />);
    expect(
      screen.getByRole('button', { name: 'Preview explanation request' }),
    ).toBeDisabled();
    expect(learning.preview).not.toHaveBeenCalled();
  });
  it('clears the password input immediately and storing a key never grants permission', async () => {
    vi.mocked(learning.setKey).mockResolvedValue();
    render(<LearningSettings />);
    const key = screen.getByLabelText('OpenAI API key');
    fireEvent.change(key, {
      target: { value: 'sk-syntheticFixture012345678901234567890' },
    });
    fireEvent.click(screen.getByRole('button', { name: 'Store in Keychain' }));
    expect(key).toHaveValue('');
    await waitFor(() => expect(learning.setKey).toHaveBeenCalledTimes(1));
    expect(learning.permission).not.toHaveBeenCalled();
    expect(learning.send).not.toHaveBeenCalled();
  });
});
