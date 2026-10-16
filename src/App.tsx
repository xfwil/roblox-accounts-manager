import { useState, useEffect, useCallback } from 'react';
import { listen } from '@tauri-apps/api/event';
import { useAccounts } from './hooks/useAccounts';
import { WorkspaceNav } from './components/WorkspaceNav';
import { Icon } from './components/Icon';
import { AccountList } from './components/AccountList';
import { JoinGameBar } from './components/JoinGameBar';
import { AddAccountDialog } from './components/AddAccountDialog';
import { JoinGameDialog } from './components/JoinGameDialog';
import { ServerListDialog } from './components/ServerListDialog';
import { AccountUtilsDialog } from './components/AccountUtilsDialog';
import { ImportExportDialog } from './components/ImportExportDialog';
import { SettingsDialog } from './components/SettingsDialog';
import { MultiLoginPage } from './components/MultiLoginPage';
import { WatcherPanel } from './components/WatcherPanel';
import * as api from './api/accounts';
import { getAccountCookie } from './api/importExport';
import { openLoginWindow } from './api/login';
import { usePresence } from './hooks/usePresence';
import type { AccountView } from './types/account';
import { getSettings } from './api/settings';
import './workspace.css';
import './shell.css';

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
  const [view, setView] = useState<'accounts' | 'multilogin'>('accounts');
  const [isSettingsOpen, setIsSettingsOpen] = useState(false);
  const [selectedAccountId, setSelectedAccountId] = useState<string | null>(null);
  const [selectedAccountIds, setSelectedAccountIds] = useState<string[]>([]);

  const [searchQuery, setSearchQuery] = useState('');
  const [selectedGroup, setSelectedGroup] = useState('All');
  const [groups, setGroups] = useState<string[]>([]);
  const [notice, setNotice] = useState<string | null>(null);

  useEffect(() => {
    getSettings().then(settings => {
      document.documentElement.dataset.theme = settings.theme;
    }).catch(() => { /* Default theme remains available without the desktop backend. */ });
  }, []);

  useEffect(() => {
    if (!notice) return;
    const timer = window.setTimeout(() => setNotice(null), 6000);
    return () => window.clearTimeout(timer);
  }, [notice]);

  const userIds = accounts.filter(a => a.user_id !== null).map(a => a.user_id as number);
  const { presenceMap } = usePresence(userIds);

  // Handle login via WebView
  const handleLogin = useCallback(async () => {
    try {
      await openLoginWindow();
    } catch (err) {
      console.error('Failed to open login window', err);
      setNotice(`Unable to open login: ${String(err)}`);
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
          setNotice(event.payload.error || 'Login failed. Please try again.');
        }
      }
    );
    return () => {
      unlisten.then((fn) => fn());
    };
  }, [fetchAccounts]);

  // Surface login page state (captcha / locked / 2sv / wrong password) detected
  // inside the Roblox login window, so the user knows why it's stuck.
  useEffect(() => {
    const messages: Record<string, string> = {
      captcha: 'Captcha detected in the login window — solve it there to continue.',
      locked: 'Account locked: Roblox wants a human check. Confirm it in the login window.',
      twostep: '2-step verification required — enter your code in the login window.',
      error: 'Login failed: incorrect username or password.',
    };
    const unlisten = listen<{ label: string; state: string }>('login-state', (event) => {
      const msg = messages[event.payload.state];
      if (msg) setNotice(msg);
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

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
  }, [accounts]);

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
      setNotice('Cookie copied. Keep it private and never share it.');
    } catch (err) {
      console.error('Failed to copy cookie', err);
      setNotice('Unable to copy cookie. Please try again.');
    }
  };

  const selectedAccount = selectedAccountId 
    ? accounts.find(a => a.id === selectedAccountId) || null 
    : null;
  const inspectedAccount = accounts.find(account => account.id === selectedAccountIds[0]);

  return (
    <div className="app-container">
      <WorkspaceNav groups={groups} selectedGroup={selectedGroup} onGroup={(g) => { setSelectedGroup(g); setView('accounts'); }} count={accounts.length} onServers={() => setIsServerListOpen(true)} onTransfer={() => setIsImportExportOpen(true)} onSettings={() => setIsSettingsOpen(true)} onMultiLogin={() => setView('multilogin')} activeView={view} />
      <div className="workspace-shell">
      <header className="workspace-header">
        <div className="workspace-breadcrumb">Workspace <span>/</span> <strong>{view === 'multilogin' ? 'Multi-login' : selectedGroup === 'All' ? 'All accounts' : selectedGroup}</strong></div>
        <span className="workspace-summary"><i className="summary-dot" />{Array.from(presenceMap.values()).filter(p => p.presenceType > 0).length} online</span>
      </header>
      {view === 'multilogin'
        ? <main className="workspace-main"><MultiLoginPage onProgress={fetchAccounts} /></main>
        : <main className="workspace-main">
      <section className="workspace-heading" aria-label="Account overview">
        <div><h1>{selectedGroup === 'All' ? 'Your accounts' : selectedGroup}</h1><p>A little organization. A lot more play.</p></div>
        <div className="heading-actions"><button className="btn btn-secondary" onClick={handleLogin}>Login with Roblox <Icon name="arrow" size={15} /></button><button className="btn btn-primary" onClick={() => setIsAddDialogOpen(true)}><Icon name="plus" size={16} />Add account</button></div>
      </section>
      <div className="workspace-columns">
      <section className="account-workspace" aria-label="Managed accounts">
      <div className="library-controls"><label className="library-search"><Icon name="search" size={17} /><input placeholder="Find an account..." aria-label="Search accounts" value={searchQuery} onChange={event => setSearchQuery(event.target.value)} /></label><button className="btn btn-ghost" onClick={refreshAll} disabled={accountsLoading}><span className={accountsLoading ? 'spin' : ''}><Icon name="refresh" size={16} /></span>Refresh</button></div>
      <select className="input-field compact-group-filter" aria-label="Filter accounts by group" value={selectedGroup} onChange={event => setSelectedGroup(event.target.value)}><option value="All">All groups</option>{groups.map(group => <option key={group} value={group}>{group}</option>)}</select>
      {accountsError && accounts.length > 0 && <div className="workspace-error" role="alert">{accountsError}</div>}

      <AccountList
        accounts={accounts}
        loading={accountsLoading}
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
        onNotice={setNotice}
      />
      <footer className="library-footer"><span>{accounts.length} total accounts</span><span>Ctrl / ⌘ + click to select multiple · Right-click for actions</span></footer>
      </section>
      <aside className="session-panel" aria-label="Session controls">
        <header className="session-heading"><span className="session-icon"><Icon name="play" size={20} /></span><h2>Launch a session</h2><p>Your next adventure starts here.</p></header>
        <div className="session-selection"><span>Launching with</span><strong>{selectedAccountIds.length ? `${selectedAccountIds.length} selected account${selectedAccountIds.length > 1 ? 's' : ''}` : 'No accounts selected'}</strong><p>{inspectedAccount ? inspectedAccount.username + (selectedAccountIds.length > 1 ? ` + ${selectedAccountIds.length - 1} more` : '') : 'Choose accounts from your library.'}</p></div>
        <JoinGameBar selectedAccountIds={selectedAccountIds} />
        <button className="server-shortcut" onClick={() => setIsServerListOpen(true)}><Icon name="server" /><span>Find a public server<small>Browse available game instances</small></span><Icon name="arrow" size={16} /></button>
        {inspectedAccount ? <section className="inspector"><h3>Account details</h3><div className="inspector-identity"><span className="account-monogram">{inspectedAccount.username.slice(0,2).toUpperCase()}</span><div><strong>{inspectedAccount.username}</strong><p>{inspectedAccount.display_name || 'Roblox account'}</p></div></div><dl><div><dt>Group</dt><dd>{inspectedAccount.group || 'Ungrouped'}</dd></div><div><dt>Balance</dt><dd>{inspectedAccount.robux === null ? 'Unavailable' : `R$ ${inspectedAccount.robux.toLocaleString()}`}</dd></div></dl><button className="btn btn-secondary" onClick={() => handleAccountUtils(inspectedAccount.id)}>Manage account<Icon name="arrow" size={14} /></button></section> : <div className="session-tip"><Icon name="accounts" /><p>Select multiple accounts to launch them together. Use Shift + click to select a range.</p></div>}
      </aside>
      </div>
      </main>}

      <WatcherPanel accounts={accounts.map(a => ({ id: a.id, username: a.username }))} />
      </div>

      {isAddDialogOpen && <AddAccountDialog
        isOpen={isAddDialogOpen}
        onClose={() => setIsAddDialogOpen(false)}
        onAdd={addAccount}
        groups={groups}
      />}

      {isJoinGameOpen && <JoinGameDialog
        key={selectedAccountId}
        isOpen={isJoinGameOpen}
        onClose={() => setIsJoinGameOpen(false)}
        accountId={selectedAccountId}
      />}

      <ServerListDialog
        isOpen={isServerListOpen}
        onClose={() => setIsServerListOpen(false)}
        accountId={selectedAccountIds[0] || null}
      />

      {isAccountUtilsOpen && <AccountUtilsDialog
        key={selectedAccountId}
        isOpen={isAccountUtilsOpen}
        onClose={() => setIsAccountUtilsOpen(false)}
        account={selectedAccount}
      />}

      <ImportExportDialog
        isOpen={isImportExportOpen}
        onClose={() => setIsImportExportOpen(false)}
        onImportComplete={() => {
          // After import, re-fetch full account list from store to pick up new accounts
          fetchAccounts();
        }}
      />

      {isSettingsOpen && <SettingsDialog
        isOpen={isSettingsOpen}
        onClose={() => setIsSettingsOpen(false)}
      />}

      {notice && <div className="workspace-toast" role="status"><span>{notice}</span><button className="btn btn-icon" aria-label="Dismiss notification" onClick={() => setNotice(null)}>×</button></div>}

    </div>
  );
}

export default App;
