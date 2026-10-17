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
export async function startBulkLogin(credentials: Credential[], concurrency: number, exportCookies: boolean, gapMs: number): Promise<void> {
  return invoke<void>('start_bulk_login', { credentials, concurrency, exportCookies, gapMs });
}

export async function cancelBulkLogin(): Promise<void> {
  return invoke<void>('cancel_bulk_login');
}

/** Bring a bulk-login window to the front (to solve a captcha/hold manually). */
export async function focusBulkWindow(index: number): Promise<void> {
  return invoke<void>('focus_bulk_window', { index });
}
