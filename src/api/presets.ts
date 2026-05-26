import { invoke } from '@tauri-apps/api/core';

export interface RecentEntry {
  value: string;
  label: string;
  timestamp: number;
}

export interface JoinPreset {
  id: string;
  name: string;
  place_id: string;
  ps_link: string;
  created_at: number;
}

// ─── Recent History ───

export async function getRecentPlaceIds(): Promise<RecentEntry[]> {
  return invoke<RecentEntry[]>('get_recent_place_ids');
}

export async function getRecentPsLinks(): Promise<RecentEntry[]> {
  return invoke<RecentEntry[]>('get_recent_ps_links');
}

export async function addRecentPlaceId(value: string, label: string): Promise<RecentEntry[]> {
  return invoke<RecentEntry[]>('add_recent_place_id', { value, label });
}

export async function addRecentPsLink(value: string, label: string): Promise<RecentEntry[]> {
  return invoke<RecentEntry[]>('add_recent_ps_link', { value, label });
}

export async function removeRecentPlaceId(value: string): Promise<RecentEntry[]> {
  return invoke<RecentEntry[]>('remove_recent_place_id', { value });
}

export async function removeRecentPsLink(value: string): Promise<RecentEntry[]> {
  return invoke<RecentEntry[]>('remove_recent_ps_link', { value });
}

// ─── Presets ───

export async function getPresets(): Promise<JoinPreset[]> {
  return invoke<JoinPreset[]>('get_presets');
}

export async function addPreset(name: string, placeId: string, psLink: string): Promise<JoinPreset[]> {
  return invoke<JoinPreset[]>('add_preset', { name, placeId, psLink });
}

export async function updatePreset(id: string, name: string, placeId: string, psLink: string): Promise<JoinPreset[]> {
  return invoke<JoinPreset[]>('update_preset', { id, name, placeId, psLink });
}

export async function removePreset(id: string): Promise<JoinPreset[]> {
  return invoke<JoinPreset[]>('remove_preset', { id });
}
