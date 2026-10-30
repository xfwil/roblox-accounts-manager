use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A Roblox account managed by RAM
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Account {
    pub id: Uuid,
    pub username: String,
    pub user_id: Option<u64>,
    pub display_name: Option<String>,
    pub description: Option<String>,
    pub robux: Option<i64>,
    pub is_premium: Option<bool>,
    pub avatar_url: Option<String>,
    /// Primary group — kept in sync with `groups[0]` for backward compatibility
    /// with features that read a single group (reports, import/export, nexus).
    pub group: Option<String>,
    /// Full group membership. Accounts saved before multi-group have this empty
    /// and only `group` set; `effective_groups()` bridges the two.
    #[serde(default)]
    pub groups: Vec<String>,
    pub alias: Option<String>,
    pub sort_order: i32,
    pub last_used: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub fields: std::collections::HashMap<String, String>,
}

/// Stored separately — the cookie is encrypted at rest
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountSecret {
    pub account_id: Uuid,
    /// AES-GCM encrypted `.ROBLOSECURITY` cookie
    pub encrypted_cookie: Vec<u8>,
    /// Nonce used for encryption
    pub nonce: Vec<u8>,
}

/// What the frontend sees (no secrets)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountView {
    pub id: Uuid,
    pub username: String,
    pub user_id: Option<u64>,
    pub display_name: Option<String>,
    pub description: Option<String>,
    pub robux: Option<i64>,
    pub is_premium: Option<bool>,
    pub avatar_url: Option<String>,
    pub group: Option<String>,
    pub groups: Vec<String>,
    pub alias: Option<String>,
    pub sort_order: i32,
    pub last_used: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub fields: std::collections::HashMap<String, String>,
}

impl Account {
    /// Normalized group membership: the explicit `groups` list, or the legacy
    /// single `group` as a one-item list for accounts saved before multi-group.
    pub fn effective_groups(&self) -> Vec<String> {
        if !self.groups.is_empty() {
            self.groups.clone()
        } else if let Some(g) = &self.group {
            vec![g.clone()]
        } else {
            Vec::new()
        }
    }

    pub fn new(username: String) -> Self {
        Self {
            id: Uuid::new_v4(),
            username,
            user_id: None,
            display_name: None,
            description: None,
            robux: None,
            is_premium: None,
            avatar_url: None,
            group: None,
            groups: Vec::new(),
            alias: None,
            sort_order: 0,
            last_used: None,
            created_at: Utc::now(),
            fields: std::collections::HashMap::new(),
        }
    }

    pub fn to_view(&self) -> AccountView {
        AccountView {
            id: self.id,
            username: self.username.clone(),
            user_id: self.user_id,
            display_name: self.display_name.clone(),
            description: self.description.clone(),
            robux: self.robux,
            is_premium: self.is_premium,
            avatar_url: self.avatar_url.clone(),
            group: self.group.clone(),
            groups: self.effective_groups(),
            alias: self.alias.clone(),
            sort_order: self.sort_order,
            last_used: self.last_used,
            created_at: self.created_at,
            fields: self.fields.clone(),
        }
    }
}
