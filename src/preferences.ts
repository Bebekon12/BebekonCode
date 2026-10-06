import { useEffect, useState } from 'react';

// Layout preferences only. Never store provider data, credentials or transcripts here:
// the core and SQLite stay the source of truth.
const prefix = 'bebekoncode.ui.';

export function usePreference<T>(key: string, initial: T) {
  const [value, setValue] = useState<T>(() => {
    try {
      const stored = localStorage.getItem(prefix + key);
      return stored === null ? initial : (JSON.parse(stored) as T);
    } catch {
      return initial;
    }
  });
  useEffect(() => {
    try {
      localStorage.setItem(prefix + key, JSON.stringify(value));
    } catch {
      /* Storage can be unavailable; the preference then lasts for this window only. */
    }
  }, [key, value]);
  return [value, setValue] as const;
}
