import { invoke } from '@tauri-apps/api/core';

export interface HomePreferences {
  schemaVersion: 1;
  homeReached: boolean;
  projectId: string | null;
}
export const defaultHomePreferences: HomePreferences = {
  schemaVersion: 1,
  homeReached: false,
  projectId: null,
};
const uuid = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;
export function parseHomePreferences(value: unknown): HomePreferences {
  if (!value || typeof value !== 'object')
    throw Error('Invalid Home preferences.');
  const v = value as Record<string, unknown>;
  if (
    v.schemaVersion !== 1 ||
    typeof v.homeReached !== 'boolean' ||
    !(
      v.projectId === null ||
      (typeof v.projectId === 'string' && uuid.test(v.projectId))
    )
  )
    throw Error('Invalid Home preferences.');
  return {
    schemaVersion: 1,
    homeReached: v.homeReached,
    projectId: v.projectId as string | null,
  };
}
export const homeNative = {
  read: async () => {
    try {
      return parseHomePreferences(await invoke('get_home_preferences'));
    } catch {
      throw Error(
        'Home preferences are unavailable. You can continue without them.',
      );
    }
  },
  save: async (preferences: HomePreferences) => {
    try {
      await invoke('save_home_preferences', {
        preferences: parseHomePreferences(preferences),
      });
    } catch {
      throw Error(
        'Your Home choice could not be saved. It may reset after restart.',
      );
    }
  },
  pickFolder: async (): Promise<string | null> => {
    try {
      const path = await invoke<unknown>('pick_project_folder');
      if (path === null) return null;
      if (
        typeof path !== 'string' ||
        path.length === 0 ||
        path.length > 4096 ||
        !path.startsWith('/') ||
        [...path].some((c) => c.charCodeAt(0) < 32 || c.charCodeAt(0) === 127)
      )
        throw Error('Invalid folder selection.');
      return path;
    } catch {
      throw Error(
        'The folder picker is unavailable. Try again or enter a folder in Sessions.',
      );
    }
  },
};
