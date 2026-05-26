import { invoke } from '@tauri-apps/api/core';
import type { LegacyImportResult } from '../types';

export async function importLegacyAccounts(
  jsonContent: string,
  validateOnline: boolean,
): Promise<LegacyImportResult[]> {
  return invoke<LegacyImportResult[]>('import_legacy_accounts', {
    jsonContent,
    validateOnline,
  });
}
