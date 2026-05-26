import { useState, useEffect, useCallback } from 'react';
import { listen } from '@tauri-apps/api/event';
import { useAccounts } from './hooks/useAccounts';
import { Toolbar } from './components/Toolbar';
import { AccountList } from './components/AccountList';
import { JoinGameBar } from './components/JoinGameBar';
import { AddAccountDialog } from './components/AddAccountDialog';
import { JoinGameDialog } from './components/JoinGameDialog';
import { ServerListDialog } from './components/ServerListDialog';
import { AccountUtilsDialog } from './components/AccountUtilsDialog';
import { ImportExportDialog } from './components/ImportExportDialog';
import { SettingsDialog } from './components/SettingsDialog';
import { WatcherPanel } from './components/WatcherPanel';
import * as api from './api/accounts';
import { getAccountCookie } from './api/importExport';
import { openLoginWindow } from './api/login';
import { usePresence } from './hooks/usePresence';
import type { AccountView } from './types/account';

function App() {
  const {
    accounts,
    loading: accountsLoading,
    error: accountsError,
    fetchAccounts,
    addAccount,
    removeAccount,
    refreshAccount,
    refreshAll,
    updateAccount
  } = useAccounts();

  const [isAddDialogOpen, setIsAddDialogOpen] = useState(false);
  const [isJoinGameOpen, setIsJoinGameOpen] = useState(false);
  const [isServerListOpen, setIsServerListOpen] = useState(false);
  const [isAccountUtilsOpen, setIsAccountUtilsOpen] = useState(false);
  const [isImportExportOpen, setIsImportExportOpen] = useState(false);
  const [isSettingsOpen, setIsSettingsOpen] = useState(false);
  const [selectedAccountId, setSelectedAccountId] = useState<string | null>(null);
  const [selectedAccountIds, setSelectedAccountIds] = useState<string[]>([]);

  const [searchQuery, setSearchQuery] = useState('');
  const [selectedGroup, setSelectedGroup] = useState('All');
  const [groups, setGroups] = useState<string[]>([]);
  const [isInitialLoad, setIsInitialLoad] = useState(true);

  const userIds = accounts.filter(a => a.user_id !== null).map(a => a.user_id as number);
  const { presenceMap } = usePresence(userIds);

  // Handle login via WebView
  const handleLogin = useCallback(async () => {
    try {
      await openLoginWindow();
    } catch (err) {
      console.error('Failed to open login window', err);
    }
  }, []);

  // Listen for login-complete event from Rust backend
  useEffect(() => {
    const unlisten = listen<{ success: boolean; account: AccountView | null; error: string | null }>(
      'login-complete',
      (event) => {
        if (event.payload.success) {
          // Refresh accounts list to include the newly added account
          fetchAccounts();
        } else {
          console.error('Login failed:', event.payload.error);
        }
      }
    );
    return () => {
      unlisten.then((fn) => fn());
    };
  }, [fetchAccounts]);

  // Fetch groups on mount and when accounts change
  useEffect(() => {
    const fetchGroups = async () => {
      try {
        const data = await api.getGroups();
        setGroups(data);
      } catch (err) {
        console.error('Failed to fetch groups', err);
      }
    };
    fetchGroups();
    setIsInitialLoad(false);
  }, [accounts.length]);

  // Actions triggered from context menu
  const handleJoinGame = (accountId: string) => {
    setSelectedAccountId(accountId);
    setIsJoinGameOpen(true);
  };

  const handleAccountUtils = (accountId: string) => {
    setSelectedAccountId(accountId);
    setIsAccountUtilsOpen(true);
  };

  const handleCopyCookie = async (accountId: string) => {
    try {
      const cookie = await getAccountCookie(accountId);
      await navigator.clipboard.writeText(cookie);
    } catch (err) {
      console.error('Failed to copy cookie', err);
      // Let it fail silently or add a toast notification system later
    }
  };

  const selectedAccount = selectedAccountId 
    ? accounts.find(a => a.id === selectedAccountId) || null 
    : null;

  return (
    <div className="app-container">
      <Toolbar
        onAddClick={() => setIsAddDialogOpen(true)}
        onLoginClick={handleLogin}
        onRefreshAll={refreshAll}
        onImportExportClick={() => setIsImportExportOpen(true)}
        onServerListClick={() => setIsServerListOpen(true)}
        onSettingsClick={() => setIsSettingsOpen(true)}
        groups={groups}
        selectedGroup={selectedGroup}
        onGroupChange={setSelectedGroup}
        searchQuery={searchQuery}
        onSearchChange={setSearchQuery}
        accountCount={accounts.length}
        loading={accountsLoading}
      />

      <JoinGameBar selectedAccountIds={selectedAccountIds} />

      <AccountList
        accounts={accounts}
        loading={accountsLoading && isInitialLoad}
        error={accountsError}
        searchQuery={searchQuery}
        selectedGroup={selectedGroup}
        groups={groups}
        presenceMap={presenceMap}
        onRefresh={refreshAccount}
        onUpdate={updateAccount}
        onRemove={removeAccount}
        onJoinGame={handleJoinGame}
        onAccountUtils={handleAccountUtils}
        onCopyCookie={handleCopyCookie}
        onSelectionChange={setSelectedAccountIds}
      />

      <WatcherPanel accounts={accounts.map(a => ({ id: a.id, username: a.username }))} />

      <AddAccountDialog
        isOpen={isAddDialogOpen}
        onClose={() => setIsAddDialogOpen(false)}
        onAdd={addAccount}
        groups={groups}
      />

      <JoinGameDialog
        isOpen={isJoinGameOpen}
        onClose={() => setIsJoinGameOpen(false)}
        accountId={selectedAccountId}
      />

      <ServerListDialog
        isOpen={isServerListOpen}
        onClose={() => setIsServerListOpen(false)}
        accountId={selectedAccountId}
      />

      <AccountUtilsDialog
        isOpen={isAccountUtilsOpen}
        onClose={() => setIsAccountUtilsOpen(false)}
        account={selectedAccount}
      />

      <ImportExportDialog
        isOpen={isImportExportOpen}
        onClose={() => setIsImportExportOpen(false)}
        onImportComplete={() => {
          // After import, re-fetch full account list from store to pick up new accounts
          fetchAccounts();
        }}
      />

      <SettingsDialog
        isOpen={isSettingsOpen}
        onClose={() => setIsSettingsOpen(false)}
      />

    </div>
  );
}

export default App;
