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
    fetchAccounts();
  }, [fetchAccounts]);

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
      throw err;
    }
  };

  const refreshAll = async () => {
    try {
      setLoading(true);
      setError(null);
      // We could either refresh each individually or list all again depending on backend impl.
      // Since there's no refresh_all in API, we'll refresh one by one or just list.
      // Assuming listAccounts does not fetch fresh data from roblox, we might need to map over all accounts.
      // For now, let's just do individual refreshes. In a real app we might want a dedicated rust command.
      const promises = accounts.map(a => api.refreshAccount(a.id));
      const updatedAccounts = await Promise.allSettled(promises);
      
      const newAccounts = [...accounts];
      updatedAccounts.forEach((result, index) => {
        if (result.status === 'fulfilled') {
          newAccounts[index] = result.value;
        }
      });
      setAccounts(newAccounts);
      
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
