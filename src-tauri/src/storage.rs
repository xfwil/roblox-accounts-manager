use std::collections::HashMap;
use std::path::PathBuf;
use uuid::Uuid;

use crate::crypto;
use crate::error::{AppError, AppResult};
use crate::models::{Account, AccountSecret};

/// File-based encrypted storage for accounts
pub struct AccountStore {
    /// Directory where account data is stored
    data_dir: PathBuf,
    /// Encryption key (derived from password or loaded from keyfile)
    encryption_key: [u8; 32],
    /// In-memory cache of accounts
    accounts: HashMap<Uuid, Account>,
    /// In-memory cache of secrets
    secrets: HashMap<Uuid, AccountSecret>,
    /// Canonical group names (lets empty groups persist for the Groups page).
    group_names: Vec<String>,
}

impl AccountStore {
    /// Create a new store, loading existing data if present
    pub fn new(data_dir: PathBuf, encryption_key: [u8; 32]) -> AppResult<Self> {
        std::fs::create_dir_all(&data_dir)?;

        let mut store = Self {
            data_dir,
            encryption_key,
            accounts: HashMap::new(),
            secrets: HashMap::new(),
            group_names: Vec::new(),
        };

        store.load()?;
        Ok(store)
    }

    /// Path to the accounts JSON file
    fn accounts_path(&self) -> PathBuf {
        self.data_dir.join("accounts.json")
    }

    /// Path to the secrets JSON file
    fn secrets_path(&self) -> PathBuf {
        self.data_dir.join("secrets.json")
    }

    /// Path to the group-names registry file
    fn groups_path(&self) -> PathBuf {
        self.data_dir.join("groups.json")
    }

    /// Load accounts and secrets from disk
    fn load(&mut self) -> AppResult<()> {
        // Load accounts
        let accounts_path = self.accounts_path();
        if accounts_path.exists() {
            let data = std::fs::read_to_string(&accounts_path)?;
            let accounts: Vec<Account> = serde_json::from_str(&data)?;
            self.accounts = accounts.into_iter().map(|a| (a.id, a)).collect();
        }

        // Load secrets
        let secrets_path = self.secrets_path();
        if secrets_path.exists() {
            let data = std::fs::read_to_string(&secrets_path)?;
            let secrets: Vec<AccountSecret> = serde_json::from_str(&data)?;
            self.secrets = secrets.into_iter().map(|s| (s.account_id, s)).collect();
        }

        // Load the group registry, or seed it from existing account memberships
        // the first time (so pre-multi-group data keeps all its groups).
        let groups_path = self.groups_path();
        if groups_path.exists() {
            let data = std::fs::read_to_string(&groups_path)?;
            self.group_names = serde_json::from_str(&data).unwrap_or_default();
        } else {
            let mut set: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
            for a in self.accounts.values() {
                for g in a.effective_groups() { set.insert(g); }
            }
            self.group_names = set.into_iter().collect();
        }

        Ok(())
    }

    /// Persist accounts and secrets to disk
    fn save(&self) -> AppResult<()> {
        // Save accounts
        let accounts: Vec<&Account> = self.accounts.values().collect();
        let data = serde_json::to_string_pretty(&accounts)?;
        std::fs::write(self.accounts_path(), data)?;

        // Save secrets
        let secrets: Vec<&AccountSecret> = self.secrets.values().collect();
        let data = serde_json::to_string_pretty(&secrets)?;
        std::fs::write(self.secrets_path(), data)?;

        // Save the group registry
        let data = serde_json::to_string_pretty(&self.group_names)?;
        std::fs::write(self.groups_path(), data)?;

        Ok(())
    }

    /// Get all accounts (sorted by sort_order)
    pub fn list_accounts(&self) -> Vec<Account> {
        let mut accounts: Vec<Account> = self.accounts.values().cloned().collect();
        accounts.sort_by_key(|a| a.sort_order);
        accounts
    }

    /// Get a single account by ID
    pub fn get_account(&self, id: &Uuid) -> AppResult<&Account> {
        self.accounts
            .get(id)
            .ok_or_else(|| AppError::AccountNotFound(id.to_string()))
    }

    /// Find an existing account by user_id
    pub fn find_by_user_id(&self, user_id: u64) -> Option<&Account> {
        self.accounts.values().find(|a| a.user_id == Some(user_id))
    }

    /// Find an existing account by username (case-insensitive)
    pub fn find_by_username(&self, username: &str) -> Option<&Account> {
        self.accounts.values().find(|a| a.username.eq_ignore_ascii_case(username))
    }

    /// Add a new account with its cookie
    pub fn add_account(&mut self, mut account: Account, cookie: &str) -> AppResult<()> {
        // Set sort order to end
        account.sort_order = self.accounts.len() as i32;

        let (encrypted_cookie, nonce) =
            crypto::encrypt_cookie(&self.encryption_key, cookie)?;

        let secret = AccountSecret {
            account_id: account.id,
            encrypted_cookie,
            nonce,
        };

        self.accounts.insert(account.id, account);
        self.secrets.insert(secret.account_id, secret);
        self.save()?;

        Ok(())
    }

    /// Add or update an account — if an account with the same user_id or username
    /// already exists, update it in-place (preserving its ID, group, alias, sort_order).
    /// Otherwise add as a new account.
    pub fn upsert_account(&mut self, account: Account, cookie: &str) -> AppResult<Uuid> {
        // Try to find existing account by user_id first, then by username
        let existing_id = account.user_id
            .and_then(|uid| self.find_by_user_id(uid).map(|a| a.id))
            .or_else(|| self.find_by_username(&account.username).map(|a| a.id));

        if let Some(id) = existing_id {
            // Update existing account — preserve user-set metadata, but bump
            // "Date Added" to now so a re-add is visible (sort by Date Added to
            // surface it) instead of silently merging into the old row.
            let existing = self.accounts.get(&id).unwrap().clone();
            let updated = Account {
                id,
                username: account.username,
                user_id: account.user_id,
                display_name: account.display_name,
                description: account.description,
                robux: account.robux,
                is_premium: account.is_premium,
                avatar_url: account.avatar_url,
                // Preserve user-set metadata
                group: existing.group,
                groups: existing.groups,
                alias: existing.alias,
                sort_order: existing.sort_order,
                last_used: existing.last_used,
                created_at: chrono::Utc::now(),
                fields: existing.fields,
            };
            self.accounts.insert(id, updated);

            // Update cookie
            let (encrypted_cookie, nonce) =
                crypto::encrypt_cookie(&self.encryption_key, cookie)?;
            let secret = AccountSecret {
                account_id: id,
                encrypted_cookie,
                nonce,
            };
            self.secrets.insert(id, secret);
            self.save()?;

            Ok(id)
        } else {
            // Add as new account
            let id = account.id;
            self.add_account(account, cookie)?;
            Ok(id)
        }
    }

    /// Set/clear a boolean marker in an account's free-form `fields` bag
    /// (e.g. "facelock", "cookie_dead"). No schema change needed; saves only
    /// when the value actually changes.
    pub fn set_flag(&mut self, id: &Uuid, key: &str, on: bool) -> AppResult<()> {
        if let Some(acc) = self.accounts.get_mut(id) {
            let changed = if on {
                acc.fields.insert(key.into(), "1".into()).as_deref() != Some("1")
            } else {
                // No longer moderated → the unlock challenge note is stale.
                if key == "moderated" {
                    acc.fields.remove("unlock_challenge");
                    acc.fields.remove("banned");
                }
                acc.fields.remove(key).is_some()
            };
            // History: count each time an account *enters* a problem state, so
            // the record survives the flag being cleared by a later good refresh.
            if on && changed {
                let n = acc.fields.get(&format!("{key}_count"))
                    .and_then(|v| v.parse::<u32>().ok())
                    .unwrap_or(0);
                acc.fields.insert(format!("{key}_count"), (n + 1).to_string());
                acc.fields.insert(format!("{key}_last"), chrono::Utc::now().to_rfc3339());
            }
            if changed {
                self.save()?;
            }
        }
        Ok(())
    }

    /// Set (Some) or remove (None) a free-form field.
    pub fn set_field(&mut self, id: &Uuid, key: &str, value: Option<&str>) -> AppResult<()> {
        if let Some(acc) = self.accounts.get_mut(id) {
            match value {
                Some(v) => { acc.fields.insert(key.into(), v.into()); }
                None => { acc.fields.remove(key); }
            }
            self.save()?;
        }
        Ok(())
    }

    /// Mark/unmark an account as face-locked. Set at login time when the cookie
    /// was harvested behind a face-lock screen, cleared on a clean login.
    pub fn set_facelock(&mut self, id: &Uuid, locked: bool) -> AppResult<()> {
        self.set_flag(id, "facelock", locked)
    }

    /// Update an existing account (metadata only, not cookie)
    pub fn update_account(&mut self, account: Account) -> AppResult<()> {
        if !self.accounts.contains_key(&account.id) {
            return Err(AppError::AccountNotFound(account.id.to_string()));
        }
        self.accounts.insert(account.id, account);
        self.save()?;
        Ok(())
    }

    /// Update the cookie for an account
    pub fn update_cookie(&mut self, account_id: &Uuid, cookie: &str) -> AppResult<()> {
        if !self.accounts.contains_key(account_id) {
            return Err(AppError::AccountNotFound(account_id.to_string()));
        }

        let (encrypted_cookie, nonce) =
            crypto::encrypt_cookie(&self.encryption_key, cookie)?;

        let secret = AccountSecret {
            account_id: *account_id,
            encrypted_cookie,
            nonce,
        };

        self.secrets.insert(*account_id, secret);
        self.save()?;
        Ok(())
    }

    /// Get the decrypted cookie for an account
    pub fn get_cookie(&self, account_id: &Uuid) -> AppResult<String> {
        let secret = self
            .secrets
            .get(account_id)
            .ok_or_else(|| AppError::AccountNotFound(account_id.to_string()))?;

        crypto::decrypt_cookie(&self.encryption_key, &secret.encrypted_cookie, &secret.nonce)
    }

    /// Remove an account and its secrets
    pub fn remove_account(&mut self, id: &Uuid) -> AppResult<()> {
        if self.accounts.remove(id).is_none() {
            return Err(AppError::AccountNotFound(id.to_string()));
        }
        self.secrets.remove(id);
        self.save()?;
        Ok(())
    }

    /// Reorder accounts by providing a list of IDs in desired order
    pub fn reorder(&mut self, order: &[Uuid]) -> AppResult<()> {
        for (i, id) in order.iter().enumerate() {
            if let Some(account) = self.accounts.get_mut(id) {
                account.sort_order = i as i32;
            }
        }
        self.save()?;
        Ok(())
    }

    /// Get all unique group names — the registry unioned with every account's
    /// live membership, so legacy groups and empty groups both show up.
    pub fn get_groups(&self) -> Vec<String> {
        let mut set: std::collections::BTreeSet<String> = self.group_names.iter().cloned().collect();
        for a in self.accounts.values() {
            for g in a.effective_groups() { set.insert(g); }
        }
        set.into_iter().collect()
    }

    /// Set an account's full group membership. `group` (primary) is kept in sync
    /// with the first entry for features that still read a single group. New
    /// names are added to the registry.
    pub fn set_account_groups(&mut self, id: &Uuid, mut groups: Vec<String>) -> AppResult<()> {
        groups.retain(|g| !g.trim().is_empty());
        groups = groups.into_iter().map(|g| g.trim().to_string()).collect();
        groups.dedup();
        if let Some(acc) = self.accounts.get_mut(id) {
            acc.group = groups.first().cloned();
            acc.groups = groups.clone();
        } else {
            return Err(AppError::AccountNotFound(id.to_string()));
        }
        for g in groups {
            if !self.group_names.contains(&g) { self.group_names.push(g); }
        }
        self.save()?;
        Ok(())
    }

    /// Create an empty group in the registry (no-op if it already exists).
    pub fn create_group(&mut self, name: &str) -> AppResult<()> {
        let name = name.trim();
        if !name.is_empty() && !self.group_names.iter().any(|g| g == name) {
            self.group_names.push(name.to_string());
            self.save()?;
        }
        Ok(())
    }

    /// Rename a group across the registry and every account that belongs to it.
    pub fn rename_group(&mut self, from: &str, to: &str) -> AppResult<()> {
        let to = to.trim();
        if from.is_empty() || to.is_empty() || from == to { return Ok(()); }
        for g in self.group_names.iter_mut() { if g == from { *g = to.to_string(); } }
        self.group_names.sort();
        self.group_names.dedup();
        self.group_names.retain(|g| g != from);
        if !self.group_names.iter().any(|g| g == to) { self.group_names.push(to.to_string()); }
        for acc in self.accounts.values_mut() {
            let mut changed = false;
            for g in acc.groups.iter_mut() { if g == from { *g = to.to_string(); changed = true; } }
            if changed { acc.groups.dedup(); }
            // Legacy single-group accounts that never got a `groups` list.
            if acc.groups.is_empty() && acc.group.as_deref() == Some(from) {
                acc.group = Some(to.to_string());
            } else if changed {
                acc.group = acc.groups.first().cloned();
            }
        }
        self.save()?;
        Ok(())
    }

    /// Delete a group from the registry and remove it from every account.
    pub fn delete_group(&mut self, name: &str) -> AppResult<()> {
        self.group_names.retain(|g| g != name);
        for acc in self.accounts.values_mut() {
            let had = acc.groups.iter().any(|g| g == name);
            acc.groups.retain(|g| g != name);
            if had {
                acc.group = acc.groups.first().cloned();
            } else if acc.groups.is_empty() && acc.group.as_deref() == Some(name) {
                // Legacy single-group account tagged only via `group`.
                acc.group = None;
            }
        }
        self.save()?;
        Ok(())
    }

    /// Get the number of accounts
    pub fn count(&self) -> usize {
        self.accounts.len()
    }
}
