import type { AccountView } from './types/account';

// Problem states the backend flags in `fields`, with a `<key>_count` / `<key>_last`
// history written each time an account enters the state.
export const PROBLEMS = [
  { key: 'facelock', label: 'Face-lock', short: 'FL', badge: 'badge-facelock' },
  { key: 'banned', label: 'Banned', short: 'Ban', badge: 'badge-banned' },
  { key: 'moderated', label: 'Moderated', short: 'Mod', badge: 'badge-moderated' },
  { key: 'cookie_dead', label: 'Cookie dead', short: 'Dead', badge: 'badge-dead' },
] as const;

export type ProblemKey = typeof PROBLEMS[number]['key'];

export const isFlagged = (acc: AccountView, key: ProblemKey) => acc.fields?.[key] === '1';

// Accounts flagged before history existed have no count yet; treat the live flag as 1.
export function historyCount(acc: AccountView, key: ProblemKey): number {
  const n = parseInt(acc.fields?.[`${key}_count`] ?? '', 10) || 0;
  return Math.max(n, isFlagged(acc, key) ? 1 : 0);
}

export function historyLast(acc: AccountView, key: ProblemKey): string | null {
  return acc.fields?.[`${key}_last`] ?? null;
}

export function lastEvent(acc: AccountView): string | null {
  const dates = PROBLEMS.map(p => historyLast(acc, p.key)).filter((d): d is string => !!d);
  return dates.sort().at(-1) ?? null;
}

/** What Roblox asked for when Unlock was pressed on a moderated account (seen in Open Browser). */
export function unlockChallenge(acc: AccountView): string | null {
  const t = acc.fields?.unlock_challenge;
  if (!t) return null;
  if (/face|bio|selfie|liveness|ident|age/.test(t)) return 'Facelock';
  if (/captcha|arkose/.test(t)) return 'Captcha Login';
  return t;
}

export const formatDay = (iso: string | null) =>
  iso ? new Date(iso).toLocaleDateString(undefined, { day: 'numeric', month: 'short', year: 'numeric' }) : '—';

/** "FL 2× · Mod 1×" — empty string when the account has no history. */
export function historySummary(acc: AccountView): string {
  return PROBLEMS.map(p => [p.short, historyCount(acc, p.key)] as const)
    .filter(([, n]) => n > 0)
    .map(([s, n]) => `${s} ${n}×`)
    .join(' · ');
}
