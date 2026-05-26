import React, { useState } from 'react';
import type { AccountView } from '../types/account';
import './AccountRow.css';

interface AccountRowProps {
  account: AccountView;
  isSelected: boolean;
  presenceType?: number;
  onClick: (e: React.MouseEvent) => void;
  onContextMenu: (e: React.MouseEvent, accountId: string) => void;
  onAliasUpdate: (id: string, newAlias: string) => Promise<void>;
  isEditingAlias: boolean;
  onStopEditingAlias: () => void;
}

// Format relative time helper
function getRelativeTime(dateString: string | null): string {
  if (!dateString) return 'Never';
  
  const date = new Date(dateString);
  const now = new Date();
  const diffInSeconds = Math.floor((now.getTime() - date.getTime()) / 1000);
  
  if (diffInSeconds < 60) return 'Just now';
  
  const diffInMinutes = Math.floor(diffInSeconds / 60);
  if (diffInMinutes < 60) return `${diffInMinutes}m ago`;
  
  const diffInHours = Math.floor(diffInMinutes / 60);
  if (diffInHours < 24) return `${diffInHours}h ago`;
  
  const diffInDays = Math.floor(diffInHours / 24);
  if (diffInDays < 30) return `${diffInDays}d ago`;
  
  const diffInMonths = Math.floor(diffInDays / 30);
  if (diffInMonths < 12) return `${diffInMonths}mo ago`;
  
  return `${Math.floor(diffInDays / 365)}y ago`;
}

function getDaysSinceUsed(dateString: string | null): number {
  if (!dateString) return Infinity;
  const date = new Date(dateString);
  const now = new Date();
  return Math.floor((now.getTime() - date.getTime()) / (1000 * 60 * 60 * 24));
}

export function AccountRow({ 
  account, 
  isSelected, 
  presenceType,
  onClick, 
  onContextMenu,
  onAliasUpdate,
  isEditingAlias,
  onStopEditingAlias
}: AccountRowProps) {
  const [tempAlias, setTempAlias] = useState(account.alias || '');
  
  const daysSinceUsed = getDaysSinceUsed(account.last_used);
  const isStale = daysSinceUsed >= 20;

  const handleAliasKeyDown = async (e: React.KeyboardEvent<HTMLInputElement>) => {
    if (e.key === 'Enter') {
      e.preventDefault();
      await onAliasUpdate(account.id, tempAlias);
      onStopEditingAlias();
    } else if (e.key === 'Escape') {
      setTempAlias(account.alias || '');
      onStopEditingAlias();
    }
  };

  const handleAliasBlur = async () => {
    if (tempAlias !== account.alias) {
      await onAliasUpdate(account.id, tempAlias);
    }
    onStopEditingAlias();
  };

  const getPresenceClass = (type?: number) => {
    switch (type) {
      case 1: return 'online';
      case 2: return 'in-game';
      case 3: return 'in-studio';
      case 0:
      default: return 'offline';
    }
  };

  const getPresenceTitle = (type?: number) => {
    switch (type) {
      case 1: return 'Online';
      case 2: return 'In Game';
      case 3: return 'In Studio';
      case 0:
      default: return 'Offline';
    }
  };

  return (
    <tr 
      className={`account-row ${isSelected ? 'selected' : ''}`}
      onClick={onClick}
      onContextMenu={(e) => onContextMenu(e, account.id)}
    >
      <td className="col-avatar">
        <div className="avatar-wrapper">
          <img 
            src={account.avatar_url || 'data:image/svg+xml;base64,PHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIHZpZXdCb3g9IjAgMCAyNCAyNCIgZmlsbD0ibm9uZSIgc3Ryb2tlPSIjYTFhMWFhIiBzdHJva2Utd2lkdGg9IjIiIHN0cm9rZS1saW5lY2FwPSJyb3VuZCIgc3Ryb2tlLWxpbmVqb2luPSJyb3VuZCI+PHBhdGggZD0iTTIwIDIxdi0yYTRgNCAwIDAgMC00LTRINGE0IDQgMCAwIDAtNCAydjIiPjwvcGF0aD48Y2lyY2xlIGN4PSIxMiIgY3k9IjciIHI9IjQiPjwvY2lyY2xlPjwvc3ZnPg=='} 
            alt={account.username} 
            className="avatar-img"
            onError={(e) => {
              (e.target as HTMLImageElement).src = 'data:image/svg+xml;base64,PHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIHZpZXdCb3g9IjAgMCAyNCAyNCIgZmlsbD0ibm9uZSIgc3Ryb2tlPSIjYTFhMWFhIiBzdHJva2Utd2lkdGg9IjIiIHN0cm9rZS1saW5lY2FwPSJyb3VuZCIgc3Ryb2tlLWxpbmVqb2luPSJyb3VuZCI+PHBhdGggZD0iTTIwIDIxdi0yYTRgNCAwIDAgMC00LTRINGE0IDQgMCAwIDAtNCAydjIiPjwvcGF0aD48Y2lyY2xlIGN4PSIxMiIgY3k9IjciIHI9IjQiPjwvY2lyY2xlPjwvc3ZnPg==';
            }}
          />
          <div 
            className={`presence-dot ${getPresenceClass(presenceType)}`}
            title={getPresenceTitle(presenceType)}
          />
        </div>
      </td>
      <td className="col-user">
        <div className="user-primary">{account.username}</div>
        <div className="user-secondary">{account.display_name}</div>
      </td>
      <td className="col-alias">
        {isEditingAlias ? (
          <input
            type="text"
            className="alias-edit-input"
            value={tempAlias}
            onChange={(e) => setTempAlias(e.target.value)}
            onKeyDown={handleAliasKeyDown}
            onBlur={handleAliasBlur}
            autoFocus
          />
        ) : (
          <span className={`alias-text ${!account.alias ? 'muted' : ''}`}>
            {account.alias || '-'}
          </span>
        )}
      </td>
      <td className="col-group">
        {account.group ? (
          <span className="badge">{account.group}</span>
        ) : (
          <span className="text-muted">-</span>
        )}
      </td>
      <td className="col-robux">
        {account.robux !== null ? (
          <span className="robux-val">R$ {account.robux.toLocaleString()}</span>
        ) : (
          <span className="text-muted">?</span>
        )}
        {account.is_premium && (
          <span className="badge badge-premium ml-sm" title="Premium">P</span>
        )}
      </td>
      <td className="col-last-used">
        <span className={isStale ? 'text-warning' : 'text-secondary'}>
          {getRelativeTime(account.last_used)}
        </span>
        {isStale && (
          <svg className="stale-icon ml-xs" width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" aria-label="Has not been used in 20+ days">
            <path d="M10.29 3.86L1.82 18a2 2 0 0 0 1.71 3h16.94a2 2 0 0 0 1.71-3L13.71 3.86a2 2 0 0 0-3.42 0z"></path>
            <line x1="12" y1="9" x2="12" y2="13"></line>
            <line x1="12" y1="17" x2="12.01" y2="17"></line>
          </svg>
        )}
      </td>
    </tr>
  );
}
