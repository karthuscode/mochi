// Rust AppInfo is authoritative. The shared fixture checks this IPC contract.
export interface AppInfo {
  readonly schemaVersion: 1;
  readonly name: string;
  readonly version: string;
}

export function parseAppInfo(value: unknown): AppInfo {
  if (
    typeof value !== 'object' ||
    value === null ||
    !('schemaVersion' in value) ||
    value.schemaVersion !== 1 ||
    !('name' in value) ||
    typeof value.name !== 'string' ||
    value.name.length === 0 ||
    !('version' in value) ||
    typeof value.version !== 'string' ||
    value.version.length === 0 ||
    Object.keys(value).length !== 3
  ) {
    throw new Error('Invalid app info response');
  }

  return { schemaVersion: 1, name: value.name, version: value.version };
}

export * from './session';
export * from './validation';
