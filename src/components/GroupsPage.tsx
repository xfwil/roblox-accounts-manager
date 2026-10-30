import { useMemo, useState } from 'react';
import type { AccountView } from '../types/account';
import { Icon } from './Icon';
import './GroupsPage.css';

interface GroupsPageProps {
  accounts: AccountView[];
  groups: string[];
  onCreate: (name: string) => Promise<void>;
  onRename: (from: string, to: string) => Promise<void>;
  onDelete: (name: string) => Promise<void>;
  onOpenGroup: (name: string) => void; // jump to the accounts view filtered by this group
}

export function GroupsPage({ accounts, groups, onCreate, onRename, onDelete, onOpenGroup }: GroupsPageProps) {
  const [newName, setNewName] = useState('');
  const [renaming, setRenaming] = useState<string | null>(null);
  const [renameValue, setRenameValue] = useState('');
  const [confirmDelete, setConfirmDelete] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  // How many accounts are in each group.
  const counts = useMemo(() => {
    const m = new Map<string, number>();
    for (const a of accounts) for (const g of a.groups ?? []) m.set(g, (m.get(g) ?? 0) + 1);
    return m;
  }, [accounts]);

  const ungrouped = useMemo(() => accounts.filter(a => !(a.groups?.length)).length, [accounts]);

  const create = async () => {
    const name = newName.trim();
    if (!name || busy) return;
    setBusy(true);
    try { await onCreate(name); setNewName(''); } finally { setBusy(false); }
  };

  const saveRename = async (from: string) => {
    const to = renameValue.trim();
    if (!to || to === from) { setRenaming(null); return; }
    setBusy(true);
    try { await onRename(from, to); setRenaming(null); } finally { setBusy(false); }
  };

  const remove = async (name: string) => {
    setBusy(true);
    try { await onDelete(name); setConfirmDelete(null); } finally { setBusy(false); }
  };

  return (
    <div className="groups-page">
      <section className="groups-heading">
        <div>
          <h1>Groups</h1>
          <p>Organize accounts into groups. An account can be in more than one. Deleting a group only removes the label — the accounts stay.</p>
        </div>
      </section>

      <section className="groups-table-card">
        <div className="groups-table-head">
          <h2>Your groups <span>{groups.length}</span></h2>
          <div className="groups-new">
            <input
              className="input-field"
              placeholder="New group name…"
              value={newName}
              onChange={e => setNewName(e.target.value)}
              onKeyDown={e => { if (e.key === 'Enter') { e.preventDefault(); void create(); } }}
            />
            <button className="btn btn-primary btn-sm" onClick={create} disabled={!newName.trim() || busy}><Icon name="plus" size={14} />New group</button>
          </div>
        </div>

        {groups.length === 0 ? (
          <p className="groups-empty">No groups yet. Type a name above and hit <strong>New group</strong>, or set groups from an account's right-click menu.</p>
        ) : (
          <div className="groups-table-scroll">
            <table className="groups-table">
              <thead>
                <tr>
                  <th className="groups-col-name">Group</th>
                  <th className="num groups-col-count">Accounts</th>
                  <th className="groups-col-actions" aria-label="Actions" />
                </tr>
              </thead>
              <tbody>
                {groups.map(g => (
                  <tr key={g}>
                    <td className="groups-name-cell">
                      <div className="groups-name-inner">
                        <span className="group-card-icon"><Icon name="folder" size={16} /></span>
                        {renaming === g ? (
                          <input
                            className="input-field group-rename"
                            autoFocus
                            value={renameValue}
                            onChange={e => setRenameValue(e.target.value)}
                            onKeyDown={e => { if (e.key === 'Enter') { e.preventDefault(); void saveRename(g); } if (e.key === 'Escape') setRenaming(null); }}
                            onBlur={() => saveRename(g)}
                          />
                        ) : (
                          <button className="group-card-name" onClick={() => onOpenGroup(g)} title="View accounts in this group">{g}</button>
                        )}
                      </div>
                    </td>
                    <td className="num text-secondary">{counts.get(g) ?? 0}</td>
                    <td className="groups-row-actions">
                      {confirmDelete === g ? (
                        <>
                          <span className="group-confirm">Delete “{g}”?</span>
                          <button className="btn btn-danger btn-sm" disabled={busy} onClick={() => remove(g)}>Delete</button>
                          <button className="btn btn-secondary btn-sm" onClick={() => setConfirmDelete(null)}>Cancel</button>
                        </>
                      ) : (
                        <>
                          <button className="btn btn-secondary btn-sm" onClick={() => { setRenaming(g); setRenameValue(g); }}>Rename</button>
                          <button className="btn btn-secondary btn-sm" onClick={() => setConfirmDelete(g)}>Delete</button>
                        </>
                      )}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}
      </section>

      <p className="groups-foot text-muted">{ungrouped} account{ungrouped === 1 ? '' : 's'} with no group.</p>
    </div>
  );
}
