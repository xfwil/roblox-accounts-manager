export interface GameServer {
  id: string;
  maxPlayers: number;
  playing: number;
  playerTokens: string[];
  fps: number;
  ping: number | null;
}

export interface GameDetails {
  placeId: number;
  name: string;
  description: string | null;
  builder: string | null;
  builderId: number | null;
  maxPlayers: number | null;
  playing: number | null;
  visits: number | null;
  favoritesCount: number | null;
  imageToken: string | null;
}

export interface LaunchOptions {
  placeId: number;
  jobId?: string;
  followUserId?: number;
  launchArgs?: string;
}

export interface PrivacySettings {
  phoneDiscovery: string | null;
  privateMessagePrivacy: string | null;
  appChatPrivacy: string | null;
  gameChatPrivacy: string | null;
  inventoryPrivacy: string | null;
  tradePrivacy: string | null;
  tradeValue: string | null;
}

export interface BirthdateResponse {
  birthMonth: number;
  birthDay: number;
  birthYear: number;
}

export interface ImportResult {
  index: number;
  success: boolean;
  username: string | null;
  error: string | null;
}

export interface ExportedAccount {
  username: string;
  cookie: string;
  group: string | null;
  alias: string | null;
}

// Phase 3 types

export interface ProcessStatus {
  account_id: string;
  pid: number;
  place_id: number;
  is_running: boolean;
  started_at: string;
  uptime_seconds: number;
}

export interface AppSettings {
  api_port: number;
  nexus_port: number;
  theme: string;
  auto_refresh_minutes: number;
  watcher_enabled: boolean;
  watcher_interval_seconds: number;
}

// Presence types

export interface UserPresence {
  userId: number;
  presenceType: number; // 0=Offline, 1=Online, 2=InGame, 3=InStudio
  lastLocation: string;
  placeId: number | null;
  gameId: string | null;
  universeId: number | null;
  lastOnline: string | null;
}

// Legacy import types

export interface LegacyImportResult {
  index: number;
  success: boolean;
  username: string | null;
  error: string | null;
  validated: boolean;
}
