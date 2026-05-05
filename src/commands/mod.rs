pub mod afk;
pub mod media;
pub mod messages;
pub mod meta;
pub mod storage;
pub mod system;

use ferogram::Client;
use ferogram::update::IncomingMessage;
use std::sync::Arc;

use crate::state::AppState;
use crate::util::esc;

pub async fn dispatch(msg: IncomingMessage, client: &Client, state: &Arc<AppState>) {
    let text = msg.text().unwrap_or_default();
    let prefix = state.config.prefix.as_str();

    if !text.starts_with(prefix) {
        return;
    }

    let rest = &text[prefix.len()..];
    let mut p = rest.splitn(2, ' ');
    let cmd = p.next().unwrap_or("").to_lowercase();
    let args = p.next().unwrap_or("").trim();

    let result: anyhow::Result<()> = match cmd.as_str() {
        "ping" => meta::ping(&msg, client).await,
        "stats" => meta::stats(&msg, client, state).await,
        "id" => meta::id(&msg).await,
        "json" => meta::json(&msg, client).await,
        "info" => meta::info(&msg, client).await,

        "purge" => messages::purge(&msg, client, state, false).await,
        "purgeme" => messages::purge(&msg, client, state, true).await,
        "copy" => messages::copy(&msg, client, args).await,
        "get" => messages::get(&msg, client, args).await,
        "fwd" => messages::fwd(&msg, client, args).await,

        "save" => storage::save(&msg, client, state).await,
        "saved" => storage::saved(&msg, client).await,

        "vta" => media::vta(&msg, client).await,
        "atv" => media::atv(&msg, client).await,

        "bash" => system::bash(&msg, client, state, args).await,
        "backup" => system::backup(&msg, client, state).await,

        "afk" => afk::set_afk(&msg, client, state, args).await,

        _ => return,
    };

    if let Err(e) = result {
        let err_text = format!(
            "❌ <b>{}</b>: <code>{}</code>",
            esc(&cmd),
            esc(&e.to_string()),
        );
        let _ = meta::edit(&msg, client, err_text).await;
    }
}
