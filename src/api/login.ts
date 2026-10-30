import { invoke } from '@tauri-apps/api/core';

export async function openLoginWindow(): Promise<void> {
  return invoke<void>('open_login_window');
}

export async function openBrowser(accountId: string, url?: string): Promise<void> {
  return invoke<void>('open_browser', { accountId, url });
}

export interface Credential {
  username: string;
  password: string;
}

/** Start bulk login. Progress arrives via the `bulk-progress` event,
 *  completion via `bulk-login-finished`. */
/** rotate: each account its own proxy · sticky: one proxy until it fails · mobile: toggle mobile data for a new IP */
export type NetMode = 'rotate' | 'sticky' | 'mobile';

export async function startBulkLogin(credentials: Credential[], concurrency: number, exportCookies: boolean, gapMs: number, proxies: string[], netMode: NetMode): Promise<void> {
  return invoke<void>('start_bulk_login', { credentials, concurrency, exportCookies, gapMs, proxies, netMode });
}

export async function cancelBulkLogin(): Promise<void> {
  return invoke<void>('cancel_bulk_login');
}

/** Bring a bulk-login window to the front (to solve a captcha/hold manually). */
export async function focusBulkWindow(index: number): Promise<void> {
  return invoke<void>('focus_bulk_window', { index });
}
