import { describe, expect, it } from 'vitest';
import fixture from '../fixtures/app-info.json';
import { parseAppInfo } from './index';

describe('app info contract', () => {
  it('accepts the shared Rust serialization fixture', () => {
    expect(parseAppInfo(fixture)).toEqual(fixture);
  });

  it.each([
    null,
    [],
    { ...fixture, schemaVersion: 2 },
    { ...fixture, name: '' },
    { ...fixture, version: 1 },
    { ...fixture, extra: true },
  ])('rejects malformed IPC data: %j', (value: unknown) => {
    expect(() => parseAppInfo(value)).toThrow('Invalid app info response');
  });
});
