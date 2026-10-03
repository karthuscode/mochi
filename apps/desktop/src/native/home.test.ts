import { mockIPC } from '@tauri-apps/api/mocks';
import { describe, expect, it } from 'vitest';
import { homeNative, parseHomePreferences } from './home';
describe('Home IPC boundary', () => {
  it.each([
    { schemaVersion: 2, homeReached: true, projectId: null },
    { schemaVersion: 1, homeReached: 'yes', projectId: null },
    { schemaVersion: 1, homeReached: true, projectId: '/private/path' },
  ])('rejects malformed preferences', (value) => {
    expect(() => parseHomePreferences(value)).toThrow();
  });
  it('keeps native preference diagnostics private', async () => {
    mockIPC(() => {
      throw Error('secret-bearing native failure');
    });
    await expect(homeNative.read()).rejects.toThrow(
      'You can continue without them',
    );
    await expect(
      homeNative.save({ schemaVersion: 1, homeReached: true, projectId: null }),
    ).rejects.toThrow('may reset after restart');
  });
  it('handles picker cancellation and validates paths without leaking errors', async () => {
    mockIPC(() => null);
    expect(await homeNative.pickFolder()).toBeNull();
    mockIPC(() => '/synthetic/project');
    expect(await homeNative.pickFolder()).toBe('/synthetic/project');
    mockIPC(() => '/synthetic/\nproject');
    await expect(homeNative.pickFolder()).rejects.toThrow(
      'folder picker is unavailable',
    );
  });
});
