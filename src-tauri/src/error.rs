use serde::Serialize;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("Account not found: {0}")]
    AccountNotFound(String),

    #[error("Encryption error: {0}")]
    Encryption(String),

    #[error("Decryption error: {0}")]
    Decryption(String),

    #[error("Storage error: {0}")]
    Storage(String),

    #[error("Roblox API error: {0}")]
    RobloxApi(String),

    #[error("HTTP request error: {0}")]
    Http(String),

    #[error("Invalid cookie: {0}")]
    InvalidCookie(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("{0}")]
    Other(String),
}

impl From<reqwest::Error> for AppError {
    fn from(e: reqwest::Error) -> Self {
        AppError::Http(e.to_string())
    }
}

impl Serialize for AppError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

pub type AppResult<T> = Result<T, AppError>;
