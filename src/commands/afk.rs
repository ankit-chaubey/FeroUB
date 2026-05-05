use anyhow::Result;
use std::sync::Arc;

use ferogram::update::IncomingMessage;
use ferogram::{Client, InputMessage};

use crate::state::AppState;
use crate::util::esc;

pub async fn set_afk(
    msg: &IncomingMessage,
    client: &Client,
    state: &Arc<AppState>,
    reason: &str,
) -> Result<()> {
    let reason_opt = if reason.is_empty() {
        None
    } else {
        Some(reason.to_string())
    };

    let display = if reason.is_empty() {
        "no reason"
    } else {
        reason
    };
    state.set_afk(reason_opt).await;

    crate::commands::meta::edit(
        msg,
        client,
        format!("💤 <b>AFK enabled</b> :  <i>{}</i>", esc(display)),
    )
    .await
}

pub async fn handle_incoming(msg: &IncomingMessage, _client: &Client, state: &Arc<AppState>) {
    if !state.is_afk().await {
        return;
    }

    let is_pm = msg.is_private();
    let is_mentioned = msg.mentioned();
    if !is_pm && !is_mentioned {
        return;
    }

    let peer_id = match msg.sender_user_id() {
        Some(id) => id,
        None => return,
    };

    if !state.afk_should_reply(peer_id) {
        return;
    }

    let reason = state.afk_reason().await;
    let base = &state.config.afk_msg;

    let text = match reason.as_deref() {
        Some(r) if !r.is_empty() => format!("💤 {} :  <i>{}</i>", esc(base), esc(r)),
        _ => format!("💤 {}", esc(base)),
    };

    let _ = msg.respond(InputMessage::html(text)).await;
}
