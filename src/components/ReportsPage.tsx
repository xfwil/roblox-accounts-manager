import { useMemo, useState } from 'react';
import { Icon } from './Icon';
import { useSort } from '../hooks/useSort';
import type { AccountView } from '../types/account';
import { PROBLEMS, historyCount, isFlagged, lastEvent, formatDay } from '../accountHistory';
import './ReportsPage.css';

interface ReportsPageProps {
  accounts: AccountView[];
  onRefresh: (ids: string[]) => Promise<void>;
  onCopyCookies: (ids: string[]) => Promise<void>;
  refreshing: boolean;
}

export function ReportsPage({ accounts, onRefresh, onCopyCookies, refreshing }: ReportsPageProps) {
  const [group, setGroup] = useState('All');
  const { sort, th } = useSort<'total' | 'username' | 'group' | 'facelock' | 'moderated' | 'cookie_dead' | 'last' | 'now'>('total');
  const groups = useMemo(() => [...new Set(accounts.map(a => a.group || 'Ungrouped'))].sort(), [accounts]);
  const list = group === 'All' ? accounts : accounts.filter(a => (a.group || 'Ungrouped') === group);

  const healthy = list.filter(a => PROBLEMS.every(p => !isFlagged(a, p.key))).length;
  const stats = PROBLEMS.map(p => ({
    ...p,
    now: list.filter(a => isFlagged(a, p.key)).length,
    ever: list.filter(a => historyCount(a, p.key) > 0).length,
    times: list.reduce((sum, a) => sum + historyCount(a, p.key), 0),
  }));

  const rows = sort(
    list
      .map(a => ({ a, counts: PROBLEMS.map(p => historyCount(a, p.key)), last: lastEvent(a) }))
      .filter(r => r.counts.some(n => n > 0)),
    (r, k) => {
      switch (k) {
        case 'total': return r.counts.reduce((s, n) => s + n, 0);
        case 'username': return r.a.username.toLowerCase();
        case 'group': return r.a.group || 'Ungrouped';
        case 'last': return r.last;
        case 'now': return PROBLEMS.find(p => isFlagged(r.a, p.key))?.label ?? 'OK';
        default: return r.counts[PROBLEMS.findIndex(p => p.key === k)];
      }
    },
  );

  return (
    <div className="reports-page">
      <section className="reports-heading">
        <div>
          <h1>Reports</h1>
          <p>How often accounts hit face-lock, moderation or a dead cookie. Moderated and cookie dead are only detected when you refresh.</p>
        </div>
        <select className="input-field reports-group" aria-label="Filter by group" value={group} onChange={e => setGroup(e.target.value)}>
          <option value="All">All groups</option>
          {groups.map(g => <option key={g} value={g}>{g}</option>)}
        </select>
      </section>

      <div className="reports-tiles">
        <div className="report-tile">
          <span className="report-label">Healthy now</span>
          <strong>{healthy}</strong>
          <small>of {list.length} accounts</small>
        </div>
        {stats.map(s => (
          <div className="report-tile" key={s.key}>
            <span className="report-label"><span className={`badge ${s.badge}`}>{s.label}</span></span>
            <strong>{s.now}</strong>
            <small>now · {s.ever} account{s.ever === 1 ? '' : 's'} ever · {s.times}× total</small>
          </div>
        ))}
      </div>

      <section className="reports-table-card">
        <div className="reports-table-head">
          <h2>Account history <span>{rows.length}</span></h2>
          {rows.length > 0 && (
            <div className="reports-actions">
              <button className="btn btn-ghost btn-sm" disabled={refreshing} onClick={() => onRefresh(rows.map(r => r.a.id))}>
                <span className={refreshing ? 'spin' : ''}><Icon name="refresh" size={14} /></span>Refresh all ({rows.length})
              </button>
              <button className="btn btn-ghost btn-sm" onClick={() => onCopyCookies(rows.map(r => r.a.id))}>
                <Icon name="copy" size={14} />Copy cookies ({rows.length})
              </button>
            </div>
          )}
        </div>
        {rows.length === 0 ? (
          <p className="reports-empty">No account has been face-locked, moderated or lost its cookie yet.</p>
        ) : (
          <div className="reports-table-scroll">
            <table className="reports-table">
              <thead>
                <tr>
                  {th('username', 'Account')}
                  {th('group', 'Group')}
                  {th('facelock', 'Face-lock', 'num')}
                  {th('moderated', 'Moderated', 'num')}
                  {th('cookie_dead', 'Cookie dead', 'num')}
                  {th('last', 'Last event')}
                  {th('now', 'Now')}
                  <th aria-label="Actions" />
                </tr>
              </thead>
              <tbody>
                {rows.map(({ a, counts, last }) => {
                  const current = PROBLEMS.find(p => isFlagged(a, p.key));
                  return (
                    <tr key={a.id}>
                      <td><strong>{a.username}</strong>{a.alias && <small> · {a.alias}</small>}</td>
                      <td className="text-secondary">{a.group || 'Ungrouped'}</td>
                      {counts.map((n, i) => <td key={PROBLEMS[i].key} className="num">{n ? `${n}×` : <span className="text-muted">—</span>}</td>)}
                      <td className="text-secondary">{formatDay(last)}</td>
                      <td>{current ? <span className={`badge ${current.badge}`}>{current.label}</span> : <span className="text-muted">OK</span>}</td>
                      <td className="reports-row-actions">
                        <button className="btn-icon" title="Refresh" aria-label={`Refresh ${a.username}`} disabled={refreshing} onClick={() => onRefresh([a.id])}><Icon name="refresh" size={15} /></button>
                        <button className="btn-icon" title="Copy cookie" aria-label={`Copy cookie of ${a.username}`} onClick={() => onCopyCookies([a.id])}><Icon name="copy" size={15} /></button>
                      </td>
                    </tr>
                  );
                })}
              </tbody>
            </table>
          </div>
        )}
      </section>
    </div>
  );
}
