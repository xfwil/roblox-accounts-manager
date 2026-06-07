import React, { useState, useMemo, useCallback, useEffect } from 'react';
import type { AccountView } from '../types/account';
import { AccountRow } from './AccountRow';
import { AccountContextMenu } from './AccountContextMenu';
import './AccountList.css';

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
  onSelectionChange
}: AccountListProps) {
  const [selectedIds, setSelectedIds] = useState<Set<string>>(new Set());
  const [editingAliasId, setEditingAliasId] = useState<string | null>(null);

  // Notify parent of selection changes
  useEffect(() => {
    onSelectionChange?.(Array.from(selectedIds));
  }, [selectedIds, onSelectionChange]);
  
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
      let cmp = 0;
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
      setContextMenu({ ...contextMenu, isOpen: false });
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
      // Range selection (simplified for now - just add to selection)
      const newSelected = new Set(selectedIds);
      newSelected.add(id);
      setSelectedIds(newSelected);
    } else {
      // Single selection
      setSelectedIds(new Set([id]));
    }
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
    setContextMenu({ ...contextMenu, isOpen: false });
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
    <div className="account-list-container" onClick={() => {
      // Clear selection when clicking empty space
      if (!contextMenu.isOpen) setSelectedIds(new Set());
      if (contextMenu.isOpen) closeContextMenu();
    }}>
      <table className="account-table">
        <thead>
          <tr>
            <th className="col-avatar"></th>
            <th className="col-user sortable-header" onClick={() => handleSort('username')}>
              User{sortIndicator('username')}
            </th>
            <th className="col-alias sortable-header" onClick={() => handleSort('alias')}>
              Alias{sortIndicator('alias')}
            </th>
            <th className="col-group sortable-header" onClick={() => handleSort('group')}>
              Group{sortIndicator('group')}
            </th>
            <th className="col-robux sortable-header" onClick={() => handleSort('robux')}>
              Robux{sortIndicator('robux')}
            </th>
            <th className="col-last-used sortable-header" onClick={() => handleSort('last_used')}>
              Last Used{sortIndicator('last_used')}
            </th>
            <th className="col-date-added sortable-header" onClick={() => handleSort('created_at')}>
              Date Added{sortIndicator('created_at')}
            </th>
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
          onRefresh={() => onRefresh(contextMenu.accountId!)}
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
              navigator.clipboard.writeText(account.username);
            }
          }}
          onJoinGame={() => onJoinGame(contextMenu.accountId!)}
          onAccountUtils={() => onAccountUtils(contextMenu.accountId!)}
          onOpenProfile={async () => {
            if (contextMenu.accountId) {
              const { openBrowser } = await import('../api/login');
              await openBrowser(contextMenu.accountId);
            }
          }}
          onRemove={() => {
            if (window.confirm('Are you sure you want to remove this account?')) {
              onRemove(contextMenu.accountId!);
            }
          }}
        />
      )}

      {/* Group Assignment Dialog */}
      {groupDialogAccountId && (
        <div className="dialog-overlay" onClick={() => setGroupDialogAccountId(null)}>
          <div className="dialog-content group-dialog" onClick={e => e.stopPropagation()}>
            <div className="dialog-header">
              <h3>Set Group</h3>
              <button className="btn-icon" onClick={() => setGroupDialogAccountId(null)}>
                <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                  <line x1="18" y1="6" x2="6" y2="18"></line>
                  <line x1="6" y1="6" x2="18" y2="18"></line>
                </svg>
              </button>
            </div>
            <div className="dialog-body">
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
                      const finalGroup = groupDialogNewGroup || groupDialogValue;
                      onUpdate(groupDialogAccountId, { group: finalGroup || undefined });
                      setGroupDialogAccountId(null);
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
                onClick={() => {
                  const finalGroup = groupDialogNewGroup || groupDialogValue;
                  onUpdate(groupDialogAccountId, { group: finalGroup || undefined });
                  setGroupDialogAccountId(null);
                }}
              >
                Save
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
