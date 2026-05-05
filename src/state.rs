use dashmap::DashSet;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;
use tokio::sync::RwLock;

use crate::config::Config;

pub struct AppState {
    pub self_id: i64,
    pub config: Config,
    pub started: Instant,
    pub msg_count: AtomicU64,
    afk: RwLock<Option<String>>,
    afk_replied: DashSet<i64>,
}

impl AppState {
    pub fn new(self_id: i64, config: Config) -> Self {
        Self {
            self_id,
            config,
            started: Instant::now(),
            msg_count: AtomicU64::new(0),
            afk: RwLock::new(None),
            afk_replied: DashSet::new(),
        }
    }

    pub fn inc_msg(&self) {
        self.msg_count.fetch_add(1, Ordering::Relaxed);
    }

    pub async fn is_afk(&self) -> bool {
        self.afk.read().await.is_some()
    }

    pub async fn afk_reason(&self) -> Option<String> {
        self.afk.read().await.clone()
    }

    pub async fn set_afk(&self, reason: Option<String>) {
        let clear = reason.is_none(); // check BEFORE the move
        *self.afk.write().await = reason;
        if clear {
            self.afk_replied.clear();
        }
    }

    pub fn afk_should_reply(&self, peer_id: i64) -> bool {
        self.afk_replied.insert(peer_id)
    }
}
