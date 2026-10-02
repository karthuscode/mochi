import { useSyncExternalStore } from 'react';

export type Appearance = 'system' | 'light' | 'dark';

function mediaSubscribe(query: string, notify: () => void) {
  const media = window.matchMedia(query);
  media.addEventListener('change', notify);
  return () => media.removeEventListener('change', notify);
}

export function useMediaPreference(query: string) {
  return useSyncExternalStore(
    (notify) => mediaSubscribe(query, notify),
    () => window.matchMedia(query).matches,
    () => false,
  );
}

function subscribeActivity(notify: () => void) {
  window.addEventListener('focus', notify);
  window.addEventListener('blur', notify);
  document.addEventListener('visibilitychange', notify);
  return () => {
    window.removeEventListener('focus', notify);
    window.removeEventListener('blur', notify);
    document.removeEventListener('visibilitychange', notify);
  };
}

export function useWindowActive() {
  return useSyncExternalStore(
    subscribeActivity,
    () => document.visibilityState === 'visible' && document.hasFocus(),
    () => false,
  );
}
