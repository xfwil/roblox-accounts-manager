import { invoke } from '@tauri-apps/api/core';
import type { GameServer, GameDetails } from '../types';

export async function joinGame(
  accountId: string,
  placeId: number,
  jobId?: string,
  followUserId?: number,
  linkCode?: string,
  launchArgs?: string,
): Promise<number> {
  return invoke<number>('join_game', {
    accountId,
    placeId,
    jobId,
    followUserId,
    linkCode,
    launchArgs,
  });
}

export async function getServers(
  placeId: number,
  cursor?: string,
  limit?: number,
): Promise<[GameServer[], string | null]> {
  return invoke<[GameServer[], string | null]>('get_servers', {
    placeId,
    cursor,
    limit,
  });
}

export async function getGameDetails(
  placeId: number,
): Promise<GameDetails[]> {
  return invoke<GameDetails[]>('get_game_details', { placeId });
}

export async function getRobloxPath(): Promise<string> {
  return invoke<string>('get_roblox_path');
}

export async function resolvePrivateServer(
  accountId: string,
  placeId: number,
  linkCode: string,
): Promise<string> {
  return invoke<string>('resolve_private_server', { accountId, placeId, linkCode });
}

export async function resolveShareLink(
  shareUrl: string,
): Promise<[number, string]> {
  return invoke<[number, string]>('resolve_share_link', { shareUrl });
}
