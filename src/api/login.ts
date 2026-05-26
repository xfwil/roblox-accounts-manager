import { invoke } from '@tauri-apps/api/core';

export async function openLoginWindow(): Promise<void> {
  return invoke<void>('open_login_window');
}

export async function openBrowser(accountId: string, url?: string): Promise<void> {
  return invoke<void>('open_browser', { accountId, url });
}
