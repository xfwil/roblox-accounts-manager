import React, { useState, useMemo, useCallback, useEffect } from 'react';
import type { AccountView } from '../types/account';
import { AccountRow } from './AccountRow';
import { AccountContextMenu } from './AccountContextMenu';
import './AccountList.css';
import { Modal } from './Modal';
import { openBrowser } from '../api/login';

type SortField = 'username' | 'alias' | 'group' | 'robux' | 'last_used' | 'created_at' | 'sort_order';
type SortDirection = 'asc' | 'desc';

interface AccountListProps {
  accounts: AccountView[];
  loading: boolean;
  error: string | null;
  searchQuery: string;
  selectedGroup: string;
  groups: string[];
  presenceMap?: Map<number, { presenceType: number }>;
  onRefresh: (id: string) => Promise<void>;
  onUpdate: (id: string, updates: {
    alias?: string;
    group?: string;
    description?: string;
    fields?: Record<string, string>;
  }) => Promise<void>;
  onRemove: (id: string) => Promise<void>;
  onJoinGame: (accountId: string) => void;
  onAccountUtils: (accountId: string) => void;
  onCopyCookie: (accountId: string) => void;
  onSelectionChange?: (ids: string[]) => void;
  onNotice?: (message: string) => void;
}

export function AccountList({
  accounts,
  loading,
  error,
  searchQuery,
  selectedGroup,
  groups,
  presenceMap,
  onRefresh,
  onUpdate,
  onRemove,
  onJoinGame,
  onAccountUtils,
  onCopyCookie,
  onSelectionChange,
  onNotice
}: AccountListProps) {
  const [selectedIds, setSelectedIds] = useState<Set<string>>(new Set());
  const [editingAliasId, setEditingAliasId] = useState<string | null>(null);
  const [selectionAnchor, setSelectionAnchor] = useState<string | null>(null);
  const [removeIds, setRemoveIds] = useState<string[]>([]);
  const [actionError, setActionError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  
  // Sort state
  const [sortField, setSortField] = useState<SortField>('sort_order');
  const [sortDirection, setSortDirection] = useState<SortDirection>('asc');

  // Group assignment dialog state
  const [groupDialogAccountId, setGroupDialogAccountId] = useState<string | null>(null);
  const [groupDialogValue, setGroupDialogValue] = useState('');
  const [groupDialogNewGroup, setGroupDialogNewGroup] = useState('');
  
  // Context menu state
  const [contextMenu, setContextMenu] = useState<{
    isOpen: boolean;
    x: number;
    y: number;
    accountId: string | null;
  }>({ isOpen: false, x: 0, y: 0, accountId: null });

  // Handle column header click for sorting
  const handleSort = useCallback((field: SortField) => {
    if (sortField === field) {
      setSortDirection(prev => prev === 'asc' ? 'desc' : 'asc');
    } else {
      setSortField(field);
      setSortDirection('asc');
    }
  }, [sortField]);

  // Filter and sort accounts
  const filteredAccounts = useMemo(() => {
    const filtered = accounts.filter(acc => {
      if (selectedGroup !== 'All' && acc.group !== selectedGroup) {
        return false;
      }
      
      if (searchQuery.trim()) {
        const query = searchQuery.toLowerCase();
        const usernameMatch = acc.username.toLowerCase().includes(query);
        const displayNameMatch = (acc.display_name || '').toLowerCase().includes(query);
        const aliasMatch = (acc.alias || '').toLowerCase().includes(query);
        
        return usernameMatch || displayNameMatch || aliasMatch;
      }
      
      return true;
    });

    // Sort
    const sorted = [...filtered].sort((a, b) => {
      let cmp: number;
      switch (sortField) {
        case 'username':
          cmp = a.username.localeCompare(b.username);
          break;
        case 'alias':
          cmp = (a.alias || '').localeCompare(b.alias || '');
          break;
        case 'group':
          cmp = (a.group || '').localeCompare(b.group || '');
          break;
        case 'robux':
          cmp = (a.robux ?? -1) - (b.robux ?? -1);
          break;
        case 'last_used': {
          const aTime = a.last_used ? new Date(a.last_used).getTime() : 0;
          const bTime = b.last_used ? new Date(b.last_used).getTime() : 0;
          cmp = aTime - bTime;
          break;
        }
        case 'created_at': {
          const aCreated = new Date(a.created_at).getTime();
          const bCreated = new Date(b.created_at).getTime();
          cmp = aCreated - bCreated;
          break;
        }
        case 'sort_order':
        default:
          cmp = a.sort_order - b.sort_order;
          break;
      }
      return sortDirection === 'asc' ? cmp : -cmp;
    });

    return sorted;
  }, [accounts, searchQuery, selectedGroup, sortField, sortDirection]);

  // Only accounts visible under the current search/group filter count as selected,
  // so hidden rows are never launched, refreshed or removed by accident.
  const visibleSelectedIds = useMemo(
    () => filteredAccounts.filter(account => selectedIds.has(account.id)).map(account => account.id),
    [filteredAccounts, selectedIds],
  );

  // Notify parent of selection changes
  useEffect(() => {
    onSelectionChange?.(visibleSelectedIds);
  }, [visibleSelectedIds, onSelectionChange]);

  // Sort indicator helper
  const sortIndicator = (field: SortField) => {
    if (sortField !== field) return null;
    return <span className="sort-arrow">{sortDirection === 'asc' ? ' \u25B2' : ' \u25BC'}</span>;
  };

  // Click handlers
  const handleRowClick = (e: React.MouseEvent, id: string) => {
    // Left click handling
    if (e.button !== 0) return;
    
    // Close context menu if open
    if (contextMenu.isOpen) {
      closeContextMenu();
    }

    if (e.ctrlKey || e.metaKey) {
      // Toggle selection
      const newSelected = new Set(selectedIds);
      if (newSelected.has(id)) {
        newSelected.delete(id);
      } else {
        newSelected.add(id);
      }
      setSelectedIds(newSelected);
    } else if (e.shiftKey && selectedIds.size > 0) {
      const newSelected = new Set(selectedIds);
      const start = filteredAccounts.findIndex(account => account.id === selectionAnchor);
      const end = filteredAccounts.findIndex(account => account.id === id);
      if (start >= 0 && end >= 0) {
        filteredAccounts.slice(Math.min(start, end), Math.max(start, end) + 1).forEach(account => newSelected.add(account.id));
      } else {
        newSelected.add(id);
      }
      setSelectedIds(newSelected);
    } else {
      // Single selection
      setSelectedIds(new Set([id]));
    }
    if (!e.shiftKey) setSelectionAnchor(id);
  };

  const handleContextMenu = (e: React.MouseEvent, id: string) => {
    e.preventDefault();
    
    // Select row if not already selected
    if (!selectedIds.has(id)) {
      setSelectedIds(new Set([id]));
    }
    
    setContextMenu({
      isOpen: true,
      x: e.clientX,
      y: e.clientY,
      accountId: id
    });
  };

  const closeContextMenu = () => {
    setContextMenu(prev => ({ ...prev, isOpen: false }));
  };

  const saveGroup = async () => {
    if (!groupDialogAccountId || busy) return;
    setBusy(true);
    setActionError(null);
    try {
      await onUpdate(groupDialogAccountId, { group: (groupDialogNewGroup || groupDialogValue).trim() });
      setGroupDialogAccountId(null);
    } catch (err) {
      setActionError(String(err));
    } finally {
      setBusy(false);
    }
  };

  // Actions
  const handleAliasUpdate = async (id: string, newAlias: string) => {
    try {
      await onUpdate(id, { alias: newAlias });
    } catch (err) {
      console.error('Failed to update alias', err);
      // Let the parent component handle error display if needed
    }
  };

  if (loading && accounts.length === 0) {
    return (
      <div className="account-list-empty">
        <div className="loading-spinner"></div>
        <p>Loading accounts...</p>
      </div>
    );
  }

  if (error && accounts.length === 0) {
    return (
      <div className="account-list-empty error-state">
        <svg width="48" height="48" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1" strokeLinecap="round" strokeLinejoin="round">
          <circle cx="12" cy="12" r="10"></circle>
          <line x1="12" y1="8" x2="12" y2="12"></line>
          <line x1="12" y1="16" x2="12.01" y2="16"></line>
        </svg>
        <p>Failed to load accounts</p>
        <p className="error-details">{error}</p>
      </div>
    );
  }

  if (filteredAccounts.length === 0) {
    return (
      <div className="account-list-empty">
        {accounts.length === 0 ? (
          <>
            <svg width="48" height="48" viewBox="0 0 24 24" fill="none" stroke="var(--text-muted)" strokeWidth="1" strokeLinecap="round" strokeLinejoin="round">
              <path d="M17 21v-2a4 4 0 0 0-4-4H5a4 4 0 0 0-4 4v2"></path>
              <circle cx="9" cy="7" r="4"></circle>
              <path d="M23 21v-2a4 4 0 0 0-3-3.87"></path>
              <path d="M16 3.13a4 4 0 0 1 0 7.75"></path>
            </svg>
            <p>No accounts found</p>
            <p className="text-muted">Click "Add Account" to get started.</p>
          </>
        ) : (
          <>
            <svg width="48" height="48" viewBox="0 0 24 24" fill="none" stroke="var(--text-muted)" strokeWidth="1" strokeLinecap="round" strokeLinejoin="round">
              <circle cx="11" cy="11" r="8"></circle>
              <line x1="21" y1="21" x2="16.65" y2="16.65"></line>
            </svg>
            <p>No matching accounts</p>
            <p className="text-muted">Try adjusting your filters or search query.</p>
          </>
        )}
      </div>
    );
  }

  return (
    <>
    <div className="selection-toolbar">
      <span>{filteredAccounts.length} accounts{visibleSelectedIds.length > 0 && ` · ${visibleSelectedIds.length} selected`}</span>
      {visibleSelectedIds.length > 0 && <div><button className="btn btn-ghost btn-sm" disabled={busy} onClick={async () => {
        setBusy(true); setActionError(null);
        try { await Promise.all(visibleSelectedIds.map(id => onRefresh(id))); } catch (err) { setActionError(String(err)); } finally { setBusy(false); }
      }}>Refresh selected</button><button className="btn btn-ghost btn-sm" onClick={() => setRemoveIds(visibleSelectedIds)}>Remove</button><button className="btn btn-ghost btn-sm" onClick={() => setSelectedIds(new Set())}>Clear</button></div>}
    </div>
    {actionError && <p className="workspace-error" role="alert">{actionError}</p>}
    <div className="account-list-container" onClick={(e) => {
      // Portaled modals bubble React clicks through here; only react to clicks inside the table area
      if (!e.currentTarget.contains(e.target as Node)) return;
      // Clear selection when clicking empty space
      if (!contextMenu.isOpen) setSelectedIds(new Set());
      if (contextMenu.isOpen) closeContextMenu();
    }}>
      <table className="account-table">
        <thead>
          <tr>
            <th className="col-avatar"><input type="checkbox" aria-label="Select all visible accounts" checked={filteredAccounts.every(account => selectedIds.has(account.id))} onClick={e => e.stopPropagation()} onChange={event => setSelectedIds(event.target.checked ? new Set(filteredAccounts.map(account => account.id)) : new Set())} /></th>
            <th className="col-user sortable-header" onClick={() => handleSort('username')}>
              <button type="button" onClick={event => { event.stopPropagation(); handleSort('username'); }}>User{sortIndicator('username')}</button>
            </th>
            <th className="col-alias sortable-header" onClick={() => handleSort('alias')}>
              <button type="button" onClick={event => { event.stopPropagation(); handleSort('alias'); }}>Alias{sortIndicator('alias')}</button>
            </th>
            <th className="col-group sortable-header" onClick={() => handleSort('group')}>
              <button type="button" onClick={event => { event.stopPropagation(); handleSort('group'); }}>Group{sortIndicator('group')}</button>
            </th>
            <th className="col-robux sortable-header" onClick={() => handleSort('robux')}>
              <button type="button" onClick={event => { event.stopPropagation(); handleSort('robux'); }}>Robux{sortIndicator('robux')}</button>
            </th>
            <th className="col-last-used sortable-header" onClick={() => handleSort('last_used')}>
              <button type="button" onClick={event => { event.stopPropagation(); handleSort('last_used'); }}>Last Used{sortIndicator('last_used')}</button>
            </th>
            <th className="col-date-added sortable-header" onClick={() => handleSort('created_at')}>
              <button type="button" onClick={event => { event.stopPropagation(); handleSort('created_at'); }}>Date Added{sortIndicator('created_at')}</button>
            </th>
            <th aria-label="Account actions" />
          </tr>
        </thead>
        <tbody>
          {filteredAccounts.map(account => {
            const presence = account.user_id ? presenceMap?.get(account.user_id) : undefined;
            return (
              <AccountRow
                key={account.id}
                account={account}
                isSelected={selectedIds.has(account.id)}
                onToggle={() => setSelectedIds(previous => { const next = new Set(previous); if (next.has(account.id)) next.delete(account.id); else next.add(account.id); return next; })}
                presenceType={presence?.presenceType}
                isEditingAlias={editingAliasId === account.id}
                onClick={(e) => {
                  e.stopPropagation();
                  handleRowClick(e, account.id);
                }}
                onContextMenu={handleContextMenu}
                onAliasUpdate={handleAliasUpdate}
                onStopEditingAlias={() => setEditingAliasId(null)}
              />
            );
          })}
        </tbody>
      </table>

      {contextMenu.accountId && (
        <AccountContextMenu
          isOpen={contextMenu.isOpen}
          x={contextMenu.x}
          y={contextMenu.y}
          onClose={closeContextMenu}
          onRefresh={() => { onRefresh(contextMenu.accountId!).catch(() => { /* error surfaced by useAccounts */ }); }}
          onEditAlias={() => setEditingAliasId(contextMenu.accountId)}
          onSetGroup={() => {
            const account = accounts.find(a => a.id === contextMenu.accountId);
            setGroupDialogValue(account?.group || '');
            setGroupDialogNewGroup('');
            setGroupDialogAccountId(contextMenu.accountId);
          }}
          onCopyCookie={() => onCopyCookie(contextMenu.accountId!)}
          onCopyUsername={() => {
            const account = accounts.find(a => a.id === contextMenu.accountId);
            if (account) {
              navigator.clipboard.writeText(account.username)
                .then(() => onNotice?.(`Copied ${account.username}`))
                .catch(() => onNotice?.('Unable to copy username.'));
            }
          }}
          onJoinGame={() => onJoinGame(contextMenu.accountId!)}
          onAccountUtils={() => onAccountUtils(contextMenu.accountId!)}
          onOpenProfile={async () => {
            if (contextMenu.accountId) {
              try {
                await openBrowser(contextMenu.accountId);
              } catch (err) {
                onNotice?.(`Unable to open browser: ${String(err)}`);
              }
            }
          }}
          onRemove={() => setRemoveIds([contextMenu.accountId!])}
        />
      )}

      {/* Group Assignment Dialog */}
      {groupDialogAccountId && (
        <Modal title="Organize account" description="Choose a group or create a new one." onClose={() => setGroupDialogAccountId(null)}>
            <div className="dialog-body">
              {actionError && <p className="alert-error" role="alert">{actionError}</p>}
              {groups.length > 0 && (
                <div className="form-group">
                  <label>Existing Groups</label>
                  <div className="group-chips">
                    <button
                      className={`group-chip ${groupDialogValue === '' ? 'active' : ''}`}
                      onClick={() => { setGroupDialogValue(''); setGroupDialogNewGroup(''); }}
                    >
                      No Group
                    </button>
                    {groups.map(g => (
                      <button
                        key={g}
                        className={`group-chip ${groupDialogValue === g ? 'active' : ''}`}
                        onClick={() => { setGroupDialogValue(g); setGroupDialogNewGroup(''); }}
                      >
                        {g}
                      </button>
                    ))}
                  </div>
                </div>
              )}
              <div className="form-group">
                <label>Or create new group</label>
                <input
                  type="text"
                  className="input-field"
                  placeholder="New group name..."
                  value={groupDialogNewGroup}
                  onChange={e => {
                    setGroupDialogNewGroup(e.target.value);
                    if (e.target.value) setGroupDialogValue(e.target.value);
                  }}
                  onKeyDown={e => {
                    if (e.key === 'Enter') {
                      e.preventDefault();
                       void saveGroup();
                    }
                  }}
                  autoFocus
                />
              </div>
            </div>
            <div className="dialog-footer">
              <button className="btn btn-secondary" onClick={() => setGroupDialogAccountId(null)}>
                Cancel
              </button>
              <button
                className="btn btn-primary"
                disabled={busy}
                onClick={saveGroup}
              >
                {busy ? 'Saving...' : 'Save'}
              </button>
            </div>
        </Modal>
      )}
    </div>
    {removeIds.length > 0 && <Modal title={`Remove ${removeIds.length === 1 ? 'account' : `${removeIds.length} accounts`}?`} description="This removes accounts from RAM, not from Roblox. You will need to add them again to use them here." onClose={() => { if (!busy) setRemoveIds([]); }}>
      <div className="dialog-body">{actionError && <p role="alert" className="alert-error">{actionError}</p>}<div className="dialog-footer"><button className="btn btn-secondary" disabled={busy} onClick={() => setRemoveIds([])}>Keep accounts</button><button className="btn btn-danger" disabled={busy} onClick={async () => {
        setBusy(true); setActionError(null);
        try { for (const id of removeIds) await onRemove(id); setSelectedIds(new Set()); setRemoveIds([]); } catch (err) { setActionError(String(err)); } finally { setBusy(false); }
      }}>{busy ? 'Removing...' : 'Remove from RAM'}</button></div></div>
    </Modal>}
    </>
  );
}
