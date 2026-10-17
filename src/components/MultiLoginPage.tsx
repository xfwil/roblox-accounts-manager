import { useEffect, useMemo, useRef, useState } from 'react';
import { listen } from '@tauri-apps/api/event';
import { Icon } from './Icon';
import { startBulkLogin, cancelBulkLogin, focusBulkWindow, type Credential } from '../api/login';
import './MultiLoginPage.css';

type RowState =
  | 'queued' | 'filling' | 'captcha' | 'locked' | 'twostep'
  | 'error' | 'done' | 'skipped' | 'cancelled';

interface Row {
  username: string;
  state: RowState;
  message: string;
  thumb?: string; // data URL from `bulk-thumb`
  flag?: RowState; // a challenge (captcha/locked/twostep) seen during login — kept even after "done"
}

interface BulkProgress { index: number; username: string; state: RowState; message: string }
interface BulkThumb { index: number; dataUrl: string }
interface BulkCookie { index: number; cookie: string }

const NEEDS_YOU: RowState[] = ['captcha', 'locked', 'twostep'];
// A login-time captcha is routine (everyone gets it) — it is NOT an account
// condition, so it's not kept as a persistent flag. Only face-lock / 2-step are.
const PERSIST_FLAG: RowState[] = ['locked', 'twostep'];
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

export function MultiLoginPage({ onProgress }: { onProgress: () => void }) {
  const [text, setText] = useState('');
  const [concurrency, setConcurrency] = useState(2);
  const [submitDelay, setSubmitDelay] = useState(0); // seconds each slot waits before picking up the next account (0 = none)
  const [running, setRunning] = useState(false);
  const [rows, setRows] = useState<Record<number, Row>>({});
  const [exportCookies, setExportCookies] = useState(false);
  const [exports, setExports] = useState<Record<number, string>>({}); // index -> "user:pass:cookie"
  const [copied, setCopied] = useState(false);
  const onProgressRef = useRef(onProgress);
  onProgressRef.current = onProgress;

  const parsed = useMemo(() => parseCredentials(text), [text]);
  const parsedRef = useRef(parsed);
  parsedRef.current = parsed;

  useEffect(() => {
    const unsubs = [
      listen<BulkProgress>('bulk-progress', (e) => {
        const { index, username, state, message } = e.payload;
        setRows((prev) => {
          const prevRow = prev[index];
          const flag = PERSIST_FLAG.includes(state) ? state : prevRow?.flag;
          return { ...prev, [index]: { ...prevRow, username, state, message, flag } };
        });
        if (state === 'done') onProgressRef.current();
      }),
      listen<BulkThumb>('bulk-thumb', (e) => {
        const { index, dataUrl } = e.payload;
        setRows((prev) => (prev[index] ? { ...prev, [index]: { ...prev[index], thumb: dataUrl } } : prev));
      }),
      listen<BulkCookie>('bulk-cookie', (e) => {
        const { index, cookie } = e.payload;
        const cred = parsedRef.current[index];
        if (cred) setExports((prev) => ({ ...prev, [index]: `${cred.username}:${cred.password}:${cookie}` }));
      }),
      listen('bulk-login-finished', () => setRunning(false)),
    ];
    return () => { unsubs.forEach((u) => u.then((fn) => fn())); };
  }, []);

  const rowList = Object.entries(rows).sort((a, b) => Number(a[0]) - Number(b[0]));
  const counts = rowList.reduce((acc, [, r]) => { acc[r.state] = (acc[r.state] ?? 0) + 1; return acc; }, {} as Record<string, number>);

  const start = async () => {
    if (!parsed.length || running) return;
    const initial: Record<number, Row> = {};
    parsed.forEach((c, i) => { initial[i] = { username: c.username, state: 'queued', message: 'Waiting…' }; });
    setRows(initial);
    setExports({});
    setCopied(false);
    setRunning(true);
    try {
      await startBulkLogin(parsed, concurrency, exportCookies, submitDelay * 1000);
    } catch (err) {
      setRunning(false);
      setRows((prev) => ({ ...prev, [-1]: { username: '—', state: 'error', message: String(err) } }));
    }
  };

  const cancel = async () => { await cancelBulkLogin(); };

  const exportText = Object.entries(exports)
    .sort((a, b) => Number(a[0]) - Number(b[0]))
    .map(([, v]) => v)
    .join('\n');
  const copyExports = async () => {
    try { await navigator.clipboard.writeText(exportText); setCopied(true); setTimeout(() => setCopied(false), 1500); } catch { /* clipboard blocked */ }
  };

  const hasRows = rowList.length > 0;

  return (
    <div className="multilogin-page">
      <section className="ml-heading">
        <div>
          <h1>Multi-login with Roblox</h1>
          <p>Log into many accounts at once. Captcha and face-lock are solved in each window; the cookie is saved the moment it appears.</p>
        </div>
      </section>

      <div className="ml-grid">
        <aside className="ml-input-card">
          <label className="ml-field">
            <span>Accounts <small>· {parsed.length} parsed</small></span>
            <textarea
              value={text}
              onChange={(e) => setText(e.target.value)}
              placeholder={'username:password\nusername,password\nusername | password'}
              spellCheck={false}
              disabled={running}
            />
          </label>
          <div className="ml-controls">
            <label className="ml-conc">
              <span>Parallel windows</span>
              <select value={concurrency} onChange={(e) => setConcurrency(Number(e.target.value))} disabled={running}>
                {[1, 2, 3, 4].map((n) => <option key={n} value={n}>{n}</option>)}
              </select>
            </label>
            <label className="ml-conc">
              <span>Delay between accounts</span>
              <select value={submitDelay} onChange={(e) => setSubmitDelay(Number(e.target.value))} disabled={running}>
                <option value={0}>None</option>
                {[1, 2, 3, 5, 8].map((n) => <option key={n} value={n}>{n}s</option>)}
              </select>
            </label>
            {running
              ? <button className="btn btn-secondary ml-action" onClick={cancel}><Icon name="close" size={15} />Cancel</button>
              : <button className="btn btn-primary ml-action" onClick={start} disabled={!parsed.length}><Icon name="arrow" size={15} />Start login</button>}
          </div>
          <label className="ml-export-toggle">
            <input type="checkbox" checked={exportCookies} onChange={(e) => setExportCookies(e.target.checked)} disabled={running} />
            <span>Export cookies <small>· as <code>user:pass:cookie</code></small></span>
          </label>
          <p className="ml-note"><Icon name="shield" size={14} /> Credentials are used once and never written to disk.</p>
          {exportCookies && Object.keys(exports).length > 0 && (
            <div className="ml-export">
              <div className="ml-export-head">
                <span>{Object.keys(exports).length} harvested</span>
                <button className="btn btn-secondary" onClick={copyExports}><Icon name="copy" size={13} />{copied ? 'Copied' : 'Copy all'}</button>
              </div>
              <textarea readOnly value={exportText} spellCheck={false} onFocus={(e) => e.currentTarget.select()} />
              <p className="ml-note"><Icon name="shield" size={14} /> Holds a live session cookie each — treat like a password.</p>
            </div>
          )}
          {hasRows && (
            <div className="ml-summary">
              {counts.done ? <span className="chip chip-done">{counts.done} added</span> : null}
              {NEEDS_YOU.some((s) => counts[s]) ? <span className="chip chip-attn">needs you</span> : null}
              {counts.error ? <span className="chip chip-error">{counts.error} failed</span> : null}
              <span className="chip">{running ? 'running…' : 'idle'}</span>
            </div>
          )}
        </aside>

        <section className="ml-status" aria-label="Login progress">
          {!hasRows ? (
            <div className="ml-empty">
              <Icon name="accounts" size={34} />
              <p>Paste accounts and hit <strong>Start login</strong>.<br />Each window shows up here live as it signs in.</p>
            </div>
          ) : (
            <div className="ml-cards">
              {rowList.map(([i, r]) => {
                const live = (['filling', 'captcha', 'locked', 'twostep'] as RowState[]).includes(r.state);
                return (
                <article
                  key={i}
                  className={`ml-card state-${r.state} ${NEEDS_YOU.includes(r.state) || r.flag ? 'attn' : ''} ${live ? 'clickable' : ''}`}
                  role={live ? 'button' : undefined}
                  tabIndex={live ? 0 : undefined}
                  title={live ? 'Click to bring this login window to the front' : undefined}
                  onClick={live ? () => focusBulkWindow(Number(i)).catch(() => {}) : undefined}
                >
                  <div className="ml-thumb">
                    {r.thumb ? <img src={r.thumb} alt="" /> : <span className="ml-thumb-ph" />}
                    <span className={`ml-badge badge-${r.state}`}>{LABEL[r.state]}</span>
                    {r.flag && r.flag !== r.state && <span className="ml-flag">{LABEL[r.flag]}</span>}
                  </div>
                  <div className="ml-card-foot">
                    <strong title={r.username}>{r.username}</strong>
                    <span className="ml-msg" title={r.message}>{r.message}</span>
                  </div>
                </article>
                );
              })}
            </div>
          )}
        </section>
      </div>
    </div>
  );
}
