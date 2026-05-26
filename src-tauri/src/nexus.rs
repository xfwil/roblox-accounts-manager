use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::broadcast;
use tokio_tungstenite::tungstenite::Message;

use crate::storage::AccountStore;

/// Nexus message types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum NexusMessage {
    /// Request to get account list
    GetAccounts,
    /// Response with account list
    AccountList {
        accounts: Vec<NexusAccount>,
    },
    /// Request to get cookie for an account
    GetCookie {
        account_id: String,
    },
    /// Response with cookie
    Cookie {
        account_id: String,
        cookie: String,
    },
    /// Launch game for an account
    LaunchAccount {
        account_id: String,
        place_id: u64,
        job_id: Option<String>,
    },
    /// Notification that an account was added/removed/updated
    AccountUpdated {
        action: String,
        account_id: String,
    },
    /// Error response
    Error {
        message: String,
    },
    /// Ping/Pong for keepalive
    Ping,
    Pong,
}

/// Simplified account info for Nexus
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NexusAccount {
    pub id: String,
    pub username: String,
    pub user_id: Option<u64>,
    pub display_name: Option<String>,
    pub group: Option<String>,
    pub alias: Option<String>,
}

/// Broadcast channel for notifying connected clients of changes
pub type NexusBroadcast = broadcast::Sender<NexusMessage>;

/// Start the Nexus WebSocket server
pub async fn start_nexus(store: Arc<Mutex<AccountStore>>, port: u16) -> NexusBroadcast {
    let (tx, _) = broadcast::channel::<NexusMessage>(100);
    let tx_clone = tx.clone();

    tokio::spawn(async move {
        let addr = SocketAddr::from(([127, 0, 0, 1], port));
        let listener = match TcpListener::bind(addr).await {
            Ok(l) => l,
            Err(e) => {
                log::error!("Failed to bind Nexus WebSocket on port {}: {}", port, e);
                return;
            }
        };

        log::info!("Nexus WebSocket server listening on ws://{}", addr);

        loop {
            match listener.accept().await {
                Ok((stream, peer)) => {
                    log::info!("Nexus: new connection from {}", peer);
                    let store = store.clone();
                    let tx = tx_clone.clone();
                    tokio::spawn(handle_connection(stream, store, tx));
                }
                Err(e) => {
                    log::error!("Nexus: accept error: {}", e);
                }
            }
        }
    });

    tx
}

async fn handle_connection(
    stream: TcpStream,
    store: Arc<Mutex<AccountStore>>,
    broadcast_tx: NexusBroadcast,
) {
    let ws_stream = match tokio_tungstenite::accept_async(stream).await {
        Ok(ws) => ws,
        Err(e) => {
            log::error!("Nexus: WebSocket handshake failed: {}", e);
            return;
        }
    };

    let (mut ws_sender, mut ws_receiver) = ws_stream.split();
    let mut broadcast_rx = broadcast_tx.subscribe();

    // Spawn a task to forward broadcast messages to this client
    let forward_task = tokio::spawn(async move {
        while let Ok(msg) = broadcast_rx.recv().await {
            if let Ok(json) = serde_json::to_string(&msg) {
                if ws_sender.send(Message::Text(json.into())).await.is_err() {
                    break;
                }
            }
        }
    });

    // Handle incoming messages from this client
    while let Some(msg) = ws_receiver.next().await {
        let msg = match msg {
            Ok(m) => m,
            Err(e) => {
                log::error!("Nexus: receive error: {}", e);
                break;
            }
        };

        match msg {
            Message::Text(text) => {
                let response = handle_message(&text, &store);
                if let Ok(json) = serde_json::to_string(&response) {
                    // Send response back via broadcast (all clients see it)
                    // For direct responses, we'd need the sender — but broadcast is simpler
                    let _ = broadcast_tx.send(response);
                    let _ = json; // response already sent via broadcast
                }
            }
            Message::Ping(data) => {
                // tokio-tungstenite handles pong automatically
                let _ = data;
            }
            Message::Close(_) => {
                log::info!("Nexus: client disconnected");
                break;
            }
            _ => {}
        }
    }

    forward_task.abort();
}

fn handle_message(text: &str, store: &Arc<Mutex<AccountStore>>) -> NexusMessage {
    let msg: NexusMessage = match serde_json::from_str(text) {
        Ok(m) => m,
        Err(e) => {
            return NexusMessage::Error {
                message: format!("Invalid message: {}", e),
            };
        }
    };

    match msg {
        NexusMessage::GetAccounts => {
            let store = match store.lock() {
                Ok(s) => s,
                Err(_) => {
                    return NexusMessage::Error {
                        message: "Lock poisoned".into(),
                    };
                }
            };

            let accounts = store
                .list_accounts()
                .iter()
                .map(|a| NexusAccount {
                    id: a.id.to_string(),
                    username: a.username.clone(),
                    user_id: a.user_id,
                    display_name: a.display_name.clone(),
                    group: a.group.clone(),
                    alias: a.alias.clone(),
                })
                .collect();

            NexusMessage::AccountList { accounts }
        }
        NexusMessage::GetCookie { account_id } => {
            let uuid = match uuid::Uuid::parse_str(&account_id) {
                Ok(u) => u,
                Err(_) => {
                    return NexusMessage::Error {
                        message: "Invalid UUID".into(),
                    };
                }
            };

            let store = match store.lock() {
                Ok(s) => s,
                Err(_) => {
                    return NexusMessage::Error {
                        message: "Lock poisoned".into(),
                    };
                }
            };

            match store.get_cookie(&uuid) {
                Ok(cookie) => NexusMessage::Cookie {
                    account_id,
                    cookie,
                },
                Err(e) => NexusMessage::Error {
                    message: e.to_string(),
                },
            }
        }
        NexusMessage::Ping => NexusMessage::Pong,
        _ => NexusMessage::Error {
            message: "Unsupported message type".into(),
        },
    }
}
