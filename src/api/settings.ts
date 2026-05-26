import { invoke } from '@tauri-apps/api/core';
import type { AppSettings } from '../types';

export async function getSettings(): Promise<AppSettings> {
  return invoke<AppSettings>('get_settings');
}

export async function saveSettings(settings: AppSettings): Promise<void> {
  return invoke<void>('save_settings', { settings });
}

export async function getThemes(): Promise<string[]> {
  return invoke<string[]>('get_themes');
}
