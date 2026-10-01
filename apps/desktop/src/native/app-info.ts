import { parseAppInfo } from '@mochi/domain';
import type { AppInfo } from '@mochi/domain';
import { invoke } from '@tauri-apps/api/core';

export async function getAppInfo(): Promise<AppInfo> {
  return parseAppInfo(await invoke<unknown>('get_app_info'));
}
