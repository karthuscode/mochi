import { mockIPC } from '@tauri-apps/api/mocks';
import { describe, expect, it } from 'vitest';
import { setCompanionVisible } from './companion';

describe('companion IPC boundary', () => {
  it('sends a bounded visibility request and validates its response', async () => {
    mockIPC((command, args) => {
      expect(command).toBe('set_companion_visible');
      expect(args).toEqual({ visible: true });
      return { schemaVersion: 1, visible: true };
    });
    await expect(setCompanionVisible(true)).resolves.toEqual({
      schemaVersion: 1,
      visible: true,
    });
  });

  it.each([
    null,
    { schemaVersion: 2, visible: true },
    { schemaVersion: 1, visible: 'yes' },
  ])('rejects a malformed native response %j', async (response) => {
    mockIPC(() => response);
    await expect(setCompanionVisible(false)).rejects.toThrow();
  });
});
