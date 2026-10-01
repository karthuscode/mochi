import { mockIPC } from '@tauri-apps/api/mocks';
import { describe, expect, it } from 'vitest';
import { learning, parseSendPreview, parseAnalysisStatus } from './learning';
import { parseDetail, parseProject } from './local-capture';
const id = '10000000-0000-4000-8000-000000000001';
describe('least privilege IPC validation', () => {
  it('rejects malformed or unbounded project and evidence responses', () => {
    expect(() =>
      parseProject({
        id,
        name: 'safe',
        root: '/synthetic',
        tracking: true,
        policyRevision: -1,
      }),
    ).toThrow();
    expect(() => parseDetail({ session: { id }, events: [] })).toThrow();
    expect(() => parseAnalysisStatus({ enabled: 'yes' })).toThrow();
  });
  it('rejects an alternate remote destination and preserves the exact bounded request', () => {
    const p = {
      token: id,
      sessionId: id,
      inputRevision: 1,
      inputHash: 'a'.repeat(64),
      requestHash: 'b'.repeat(64),
      provider: 'OpenAI',
      endpoint: 'https://api.openai.com/v1/responses',
      model: 'gpt-4o-mini-2024-07-18',
      purpose: 'generation',
      requestJson: '{}',
      expiresInSeconds: 300,
    };
    expect(parseSendPreview(p).requestJson).toBe('{}');
    expect(() => parseSendPreview({ ...p, requestJson: '{invalid' })).toThrow();
    expect(() => parseSendPreview({ ...p, requestJson: '[]' })).toThrow();
    expect(() =>
      parseSendPreview({ ...p, endpoint: 'https://attacker.invalid' }),
    ).toThrow();
    expect(() =>
      parseSendPreview({ ...p, requestJson: 'x'.repeat(128 * 1024 + 1) }),
    ).toThrow();
  });
  it('discards untrusted native diagnostic content and allows only fixed public errors', async () => {
    mockIPC(() => {
      throw 'PRIVATE_PATH_OR_PAYLOAD';
    });
    await expect(learning.preview(id)).rejects.toThrow(
      'could not be completed safely',
    );
    try {
      await learning.preview(id);
    } catch (error) {
      expect(String(error)).not.toContain('PRIVATE_PATH_OR_PAYLOAD');
    }
    mockIPC(() => {
      throw 'Finish the observed session before preparing analysis.';
    });
    await expect(learning.preview(id)).rejects.toThrow(
      'Finish the observed session',
    );
  });
});
