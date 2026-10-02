import { invoke } from '@tauri-apps/api/core';

export interface CompanionTrial {
  readonly schemaVersion: 1;
  readonly visible: boolean;
}

function parseTrial(value: unknown): CompanionTrial {
  if (
    typeof value !== 'object' ||
    value === null ||
    !('schemaVersion' in value) ||
    value.schemaVersion !== 1 ||
    !('visible' in value) ||
    typeof value.visible !== 'boolean'
  ) {
    throw new Error('Companion response is unavailable.');
  }
  return { schemaVersion: 1, visible: value.visible };
}

export async function setCompanionVisible(
  visible: boolean,
): Promise<CompanionTrial> {
  return parseTrial(
    await invoke<unknown>('set_companion_visible', { visible }),
  );
}

export async function openMochi(): Promise<void> {
  await invoke('open_mochi');
}

export async function hideCompanion(): Promise<void> {
  await invoke('hide_companion');
}

export async function startCompanionDrag(): Promise<void> {
  await invoke('start_companion_drag');
}
