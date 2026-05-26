import { invoke } from '@tauri-apps/api/core';
import type { UserPresence } from '../types';

export async function getAccountPresences(): Promise<UserPresence[]> {
  return invoke<UserPresence[]>('get_account_presences');
}

export async function getSinglePresence(accountId: string): Promise<UserPresence> {
  return invoke<UserPresence>('get_single_presence', { accountId });
}
