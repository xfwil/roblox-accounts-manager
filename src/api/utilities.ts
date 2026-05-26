import { invoke } from '@tauri-apps/api/core';
import type { PrivacySettings, BirthdateResponse } from '../types';

export async function changePassword(
  accountId: string,
  oldPassword: string,
  newPassword: string,
): Promise<void> {
  return invoke<void>('change_password', {
    accountId,
    oldPassword,
    newPassword,
  });
}

export async function changeEmail(
  accountId: string,
  password: string,
  newEmail: string,
): Promise<void> {
  return invoke<void>('change_email', {
    accountId,
    password,
    newEmail,
  });
}

export async function getPrivacySettings(
  accountId: string,
): Promise<PrivacySettings> {
  return invoke<PrivacySettings>('get_privacy_settings', { accountId });
}

export async function setAccountDescription(
  accountId: string,
  description: string,
): Promise<void> {
  return invoke<void>('set_account_description', {
    accountId,
    description,
  });
}

export async function setDisplayName(
  accountId: string,
  newDisplayName: string,
): Promise<void> {
  return invoke<void>('set_display_name', {
    accountId,
    newDisplayName,
  });
}

export async function getGender(accountId: string): Promise<number> {
  return invoke<number>('get_gender', { accountId });
}

export async function getBirthdate(
  accountId: string,
): Promise<BirthdateResponse> {
  return invoke<BirthdateResponse>('get_birthdate', { accountId });
}
