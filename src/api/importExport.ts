import { invoke } from '@tauri-apps/api/core';
import type { ImportResult } from '../types';

export async function importCookies(
  cookies: string[],
  group?: string,
): Promise<ImportResult[]> {
  return invoke<ImportResult[]>('import_cookies', { cookies, group });
}

export async function importUserPassCookie(
  entries: string[],
  group?: string,
): Promise<ImportResult[]> {
  return invoke<ImportResult[]>('import_user_pass_cookie', { entries, group });
}

export async function exportAccounts(
  format: 'cookies' | 'user:cookie' | 'json',
): Promise<string> {
  return invoke<string>('export_accounts', { format });
}

export async function getAccountCookie(
  accountId: string,
): Promise<string> {
  return invoke<string>('get_account_cookie', { accountId });
}

export async function updateAccountCookie(
  accountId: string,
  newCookie: string,
): Promise<void> {
  return invoke<void>('update_account_cookie', { accountId, newCookie });
}
