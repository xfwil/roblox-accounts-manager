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

    /// Get all unique group names
    pub fn get_groups(&self) -> Vec<String> {
        let mut groups: Vec<String> = self
            .accounts
            .values()
            .filter_map(|a| a.group.clone())
            .collect();
        groups.sort();
        groups.dedup();
        groups
    }

    /// Get the number of accounts
    pub fn count(&self) -> usize {
        self.accounts.len()
    }
}
