import { useEffect, useMemo, useRef, useState } from 'react';
import { listen } from '@tauri-apps/api/event';
import { Modal } from './Modal';
import { startBulkLogin, cancelBulkLogin, type Credential } from '../api/login';
import './BulkLoginDialog.css';

type RowState =
  | 'queued' | 'filling' | 'captcha' | 'locked' | 'twostep'
  | 'error' | 'done' | 'skipped' | 'cancelled';

interface Row {
  username: string;
  state: RowState;
  message: string;
  thumb?: string; // data URL, fed by `bulk-thumb`
}

interface BulkProgress { index: number; username: string; state: RowState; message: string }
interface BulkThumb { index: number; dataUrl: string }

/** States that need the user to act in the window. */
const NEEDS_YOU: RowState[] = ['captcha', 'locked', 'twostep'];
const LABEL: Record<RowState, string> = {
  queued: 'Queued', filling: 'Signing in', captcha: 'Captcha', locked: 'Face-lock',
  twostep: '2-step', error: 'Failed', done: 'Added', skipped: 'Skipped', cancelled: 'Cancelled',
};

function parseCredentials(text: string): Credential[] {
  return text
    .split(/\r?\n/)
    .map((l) => l.trim())
    .filter(Boolean)
    .map((line) => {
      const sep = line.search(/[:,|\t]/); // user:pass, user,pass, user|pass, user<tab>pass
      if (sep < 0) return null;
      const username = line.slice(0, sep).trim();
      const password = line.slice(sep + 1).trim();
      return username && password ? { username, password } : null;
    })
    .filter((c): c is Credential => c !== null);
}

export function BulkLoginDialog({ isOpen, onClose, onProgress }: {
  isOpen: boolean;
  onClose: () => void;
  onProgress: () => void; // refresh accounts list when one is added
}) {
  const [text, setText] = useState('');
  const [concurrency, setConcurrency] = useState(2);
  const [running, setRunning] = useState(false);
  const [rows, setRows] = useState<Record<number, Row>>({});
  const onProgressRef = useRef(onProgress);
  onProgressRef.current = onProgress;

  const parsed = useMemo(() => parseCredentials(text), [text]);

  // Listen for backend progress + thumbnails while the dialog is mounted.
  useEffect(() => {
    if (!isOpen) return;
    const unsubs = [
      listen<BulkProgress>('bulk-progress', (e) => {
        const { index, username, state, message } = e.payload;
        setRows((prev) => ({ ...prev, [index]: { ...prev[index], username, state, message } }));
        if (state === 'done') onProgressRef.current();
      }),
      listen<BulkThumb>('bulk-thumb', (e) => {
        const { index, dataUrl } = e.payload;
        setRows((prev) => (prev[index] ? { ...prev, [index]: { ...prev[index], thumb: dataUrl } } : prev));
      }),
      listen('bulk-login-finished', () => setRunning(false)),
    ];
    return () => { unsubs.forEach((u) => u.then((fn) => fn())); };
  }, [isOpen]);

  if (!isOpen) return null;

  const counts = Object.values(rows).reduce((acc, r) => {
    acc[r.state] = (acc[r.state] ?? 0) + 1;
    return acc;
  }, {} as Record<string, number>);

  const start = async () => {
    if (!parsed.length || running) return;
    const initial: Record<number, Row> = {};
    parsed.forEach((c, i) => { initial[i] = { username: c.username, state: 'queued', message: 'Waiting…' }; });
    setRows(initial);
    setRunning(true);
    try {
      await startBulkLogin(parsed, concurrency);
    } catch (err) {
      setRunning(false);
      setRows((prev) => ({ ...prev, [-1]: { username: '—', state: 'error', message: String(err) } }));
    }
  };

  const cancel = async () => { await cancelBulkLogin(); };

  const hasRows = Object.keys(rows).length > 0;

  return (
    <Modal
      title="Multi-login with Roblox"
      description="Paste accounts as username:password, one per line. Captcha and face-lock are solved in each window; the cookie is saved as soon as it appears."
      onClose={onClose}
      className="modal-wide bulk-login-modal"
    >
      <div className="bulk-body">
        <section className="bulk-input">
          <label className="bulk-field">
            <span>Accounts <small>({parsed.length} parsed)</small></span>
            <textarea
              value={text}
              onChange={(e) => setText(e.target.value)}
              placeholder={'user1:password1\nuser2:password2'}
              spellCheck={false}
              disabled={running}
              rows={8}
            />
          </label>
          <div className="bulk-controls">
            <label className="bulk-conc">
              Parallel windows
              <select value={concurrency} onChange={(e) => setConcurrency(Number(e.target.value))} disabled={running}>
                {[1, 2, 3, 4].map((n) => <option key={n} value={n}>{n}</option>)}
              </select>
            </label>
            {running
              ? <button className="btn btn-secondary" onClick={cancel}>Cancel</button>
              : <button className="btn btn-primary" onClick={start} disabled={!parsed.length}>Start login</button>}
          </div>
          <p className="bulk-note">Credentials are used once and never written to disk.</p>
        </section>

        {hasRows && (
          <section className="bulk-status">
            <div className="bulk-summary">
              {counts.done ? <span className="chip chip-done">{counts.done} added</span> : null}
              {NEEDS_YOU.some((s) => counts[s]) ? <span className="chip chip-attn">needs you</span> : null}
              {counts.error ? <span className="chip chip-error">{counts.error} failed</span> : null}
              {running ? <span className="chip">running…</span> : <span className="chip">done</span>}
            </div>
            <ul className="bulk-rows">
              {Object.entries(rows).map(([i, r]) => (
                <li key={i} className={`bulk-row state-${r.state} ${NEEDS_YOU.includes(r.state) ? 'attn' : ''}`}>
                  <div className="bulk-thumb">
                    {r.thumb ? <img src={r.thumb} alt="" /> : <span className="bulk-thumb-ph" />}
                  </div>
                  <div className="bulk-row-main">
                    <strong>{r.username}</strong>
                    <span className="bulk-row-msg">{r.message}</span>
                  </div>
                  <span className={`bulk-badge badge-${r.state}`}>{LABEL[r.state]}</span>
                </li>
              ))}
            </ul>
          </section>
        )}
      </div>
    </Modal>
  );
}
