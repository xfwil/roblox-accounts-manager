import { invoke } from '@tauri-apps/api/core';
import type { ProcessStatus } from '../types';

export async function trackProcess(
  pid: number,
  accountId: string,
  placeId: number,
): Promise<void> {
  return invoke<void>('track_process', { pid, accountId, placeId });
}

export async function untrackProcess(pid: number): Promise<void> {
  return invoke<void>('untrack_process', { pid });
}

export async function getProcessStatus(): Promise<ProcessStatus[]> {
  return invoke<ProcessStatus[]>('get_process_status');
}

export async function cleanupDeadProcesses(): Promise<number> {
  return invoke<number>('cleanup_dead_processes');
}

export async function getRunningCount(): Promise<number> {
  return invoke<number>('get_running_count');
}

export async function findRobloxProcesses(): Promise<[number, string][]> {
  return invoke<[number, string][]>('find_roblox_processes');
}
