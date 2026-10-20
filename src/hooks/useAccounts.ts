import { useState, useEffect, useCallback } from 'react';
import type { AccountView } from '../types/account';
import * as api from '../api/accounts';

export function useAccounts() {
  const [accounts, setAccounts] = useState<AccountView[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const fetchAccounts = useCallback(async () => {
    try {
      setLoading(true);
      setError(null);
      const data = await api.listAccounts();
      setAccounts(data);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    let active = true;
    api.listAccounts().then(data => { if (active) setAccounts(data); })
      .catch(err => { if (active) setError(String(err)); })
      .finally(() => { if (active) setLoading(false); });
    return () => { active = false; };
  }, []);

  const addAccount = async (cookie: string, group?: string, alias?: string) => {
    try {
      setError(null);
      await api.addAccount(cookie, group, alias);
      await fetchAccounts();
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      setError(`Failed to add account: ${msg}`);
      throw err;
    }
  };

  const removeAccount = async (id: string) => {
    try {
      setError(null);
      await api.removeAccount(id);
      setAccounts(prev => prev.filter(a => a.id !== id));
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      setError(`Failed to remove account: ${msg}`);
      throw err;
    }
  };

  const refreshAccount = async (id: string) => {
    try {
      setError(null);
      const updatedAccount = await api.refreshAccount(id);
      setAccounts(prev => prev.map(a => (a.id === id ? updatedAccount : a)));
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      setError(`Failed to refresh account: ${msg}`);
      // Backend may have flagged the cookie dead — pull fresh views so the row shows it.
      await fetchAccounts();
      throw err;
    }
  };

  // ids = refresh only these accounts (default: all)
  const refreshAll = async (ids?: string[]) => {
    const targets = ids ? accounts.filter(a => ids.includes(a.id)) : accounts;
    try {
      setLoading(true);
      setError(null);
      // Refresh in small batches — firing all accounts at once floods Roblox
      // from one IP and trips rate-limit / bot-protection, which came back as
      // false "cookie dead" flags on perfectly valid cookies.
      const POOL = 5;
      const sleep = (ms: number) => new Promise(r => setTimeout(r, ms));
      let failed = 0;
      for (let i = 0; i < targets.length; i += POOL) {
        const batch = targets.slice(i, i + POOL);
        const results = await Promise.allSettled(batch.map(a => api.refreshAccount(a.id)));
        failed += results.filter(result => result.status === 'rejected').length;
        if (i + POOL < targets.length) await sleep(250);
      }
      // Each refresh persists its result (incl. a cookie-dead marker on failure);
      // re-list so every row reflects the stored state, failures included.
      await fetchAccounts();
      if (failed > 0) setError(`${failed} of ${targets.length} accounts failed to refresh — check the Status column (Moderated / Cookie dead)`);

    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      setError(`Failed to refresh all: ${msg}`);
    } finally {
      setLoading(false);
    }
  };

  const updateAccount = async (id: string, updates: Parameters<typeof api.updateAccount>[1]) => {
     try {
      setError(null);
      const updatedAccount = await api.updateAccount(id, updates);
      setAccounts(prev => prev.map(a => (a.id === id ? updatedAccount : a)));
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      setError(`Failed to update account: ${msg}`);
      throw err;
    }
  }

  const reorderAccounts = async (ids: string[]) => {
    try {
        setError(null);
        await api.reorderAccounts(ids);
        // Optimistic update - sort current array to match provided ids
        setAccounts(prev => {
            const sorted = [...prev].sort((a, b) => {
                const indexA = ids.indexOf(a.id);
                const indexB = ids.indexOf(b.id);
                return (indexA === -1 ? Infinity : indexA) - (indexB === -1 ? Infinity : indexB);
            });
            return sorted;
        });
    } catch (err) {
        const msg = err instanceof Error ? err.message : String(err);
        setError(`Failed to reorder accounts: ${msg}`);
        throw err;
    }
  }

  return {
    accounts,
    loading,
    error,
    fetchAccounts,
    addAccount,
    removeAccount,
    refreshAccount,
    refreshAll,
    updateAccount,
    reorderAccounts
  };
}
