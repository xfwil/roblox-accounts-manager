import { useState, useEffect, useCallback, useRef } from 'react';
import { getAccountPresences } from '../api/presence';
import type { UserPresence } from '../types';

const POLL_INTERVAL_MS = 120_000; // 2 minutes

/**
 * Hook that polls Roblox presence API for all accounts.
 * Returns a map of userId -> UserPresence.
 */
export function usePresence(userIds: number[], enabled: boolean = true) {
  const [presenceMap, setPresenceMap] = useState<Map<number, UserPresence>>(new Map());
  const [loading, setLoading] = useState(false);
  const intervalRef = useRef<ReturnType<typeof setInterval> | null>(null);

  const fetchPresences = useCallback(async () => {
    if (userIds.length === 0) return;

    try {
      setLoading(true);
      const presences = await getAccountPresences();
      const map = new Map<number, UserPresence>();
      for (const p of presences) {
        map.set(p.userId, p);
      }
      setPresenceMap(map);
    } catch (err) {
      console.error('Failed to fetch presences:', err);
    } finally {
      setLoading(false);
    }
  }, [userIds.length]);

  useEffect(() => {
    if (!enabled || userIds.length === 0) return;

    // Initial fetch
    fetchPresences();

    // Poll every 2 minutes
    intervalRef.current = setInterval(fetchPresences, POLL_INTERVAL_MS);

    return () => {
      if (intervalRef.current) {
        clearInterval(intervalRef.current);
        intervalRef.current = null;
      }
    };
  }, [enabled, fetchPresences, userIds.length]);

  return { presenceMap, loading, refresh: fetchPresences };
}
