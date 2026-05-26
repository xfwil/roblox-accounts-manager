import { useState, useCallback, useEffect } from 'react';
import {
  getRecentPlaceIds,
  getRecentPsLinks,
  addRecentPlaceId,
  addRecentPsLink,
  removeRecentPlaceId,
  removeRecentPsLink,
  type RecentEntry,
} from '../api/presets';

type HistoryKind = 'place' | 'ps';

/**
 * Hook for managing recent input history stored via Tauri backend.
 * Persists across dev/release modes (file-based, not localStorage).
 */
export function useRecentHistory(kind: HistoryKind) {
  const [entries, setEntries] = useState<RecentEntry[]>([]);

  // Load on mount
  useEffect(() => {
    const load = kind === 'place' ? getRecentPlaceIds : getRecentPsLinks;
    load().then(setEntries).catch(console.error);
  }, [kind]);

  const addEntry = useCallback(
    async (value: string, label?: string) => {
      const trimmed = value.trim();
      if (!trimmed) return;

      const add = kind === 'place' ? addRecentPlaceId : addRecentPsLink;
      try {
        const updated = await add(trimmed, label || trimmed);
        setEntries(updated);
      } catch (err) {
        console.error('Failed to add recent entry:', err);
      }
    },
    [kind],
  );

  const removeEntry = useCallback(
    async (value: string) => {
      const remove = kind === 'place' ? removeRecentPlaceId : removeRecentPsLink;
      try {
        const updated = await remove(value);
        setEntries(updated);
      } catch (err) {
        console.error('Failed to remove recent entry:', err);
      }
    },
    [kind],
  );

  return { entries, addEntry, removeEntry };
}
