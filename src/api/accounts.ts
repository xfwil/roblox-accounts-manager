import { invoke } from '@tauri-apps/api/core';
import type { AccountView } from '../types';

export async function listAccounts(): Promise<AccountView[]> {
  return invoke<AccountView[]>('list_accounts');
}

export async function getAccount(id: string): Promise<AccountView> {
  return invoke<AccountView>('get_account', { id });
}

export async function addAccount(
  cookie: string,
  group?: string,
  alias?: string,
): Promise<AccountView> {
  return invoke<AccountView>('add_account', { cookie, group, alias });
}

export async function removeAccount(id: string): Promise<void> {
  return invoke<void>('remove_account', { id });
}

export async function updateAccount(
  id: string,
  updates: {
    alias?: string;
    group?: string;
    description?: string;
    fields?: Record<string, string>;
  },
): Promise<AccountView> {
  return invoke<AccountView>('update_account', { id, ...updates });
}

export async function refreshAccount(id: string): Promise<AccountView> {
  return invoke<AccountView>('refresh_account', { id });
}

export async function reorderAccounts(ids: string[]): Promise<void> {
  return invoke<void>('reorder_accounts', { ids });
}

export async function getGroups(): Promise<string[]> {
  return invoke<string[]>('get_groups');
}

export async function getAccountCount(): Promise<number> {
  return invoke<number>('get_account_count');
}
