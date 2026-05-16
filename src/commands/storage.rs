use anyhow::Result;
use std::path::PathBuf;
use std::sync::Arc;

use ferogram::Client;
use ferogram::tl;
use ferogram::update::IncomingMessage;

use crate::state::AppState;
use crate::util::{esc, random_id};

pub async fn save(msg: &IncomingMessage, client: &Client, state: &Arc<AppState>) -> Result<()> {
    let target_id = msg
        .reply_to_message_id()
        .ok_or_else(|| anyhow::anyhow!("reply to a message with media to save it"))?;

    let resp = client
        .invoke(&tl::functions::messages::GetMessages {
            id: vec![tl::enums::InputMessage::Id(tl::types::InputMessageId {
                id: target_id,
            })],
        })
        .await?;

    let msgs = crate::util::messages_from_response(resp);
    let source = msgs
        .into_iter()
        .find_map(|m| match m {
            tl::enums::Message::Message(m) => Some(m),
            _ => None,
        })
        .ok_or_else(|| anyhow::anyhow!("target message not found"))?;

    let media = source
        .media
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("replied message has no media"))?;

    std::fs::create_dir_all(&state.config.save_dir)?;

    let filename = filename_for_media(media, target_id);
    let path: PathBuf = [&state.config.save_dir, &filename].iter().collect();

    client.download_file(media, &path).await?;

    let abs_path = std::fs::canonicalize(&path).unwrap_or(path);
    let abs_str = abs_path.display().to_string();

    crate::commands::meta::edit(
        msg,
        client,
        format!("💾 <b>Saved</b>\n<code>{}</code>", esc(&abs_str)),
    )
    .await
}

pub async fn saved(msg: &IncomingMessage, client: &Client) -> Result<()> {
    let target_id = msg
        .reply_to_message_id()
        .ok_or_else(|| anyhow::anyhow!("reply to a message to forward it to Saved Messages"))?;

    let from_peer = crate::util::peer_from_id(msg.chat_id());

    client
        .invoke(&tl::functions::messages::ForwardMessages {
            silent: true,
            background: false,
            with_my_score: false,
            drop_author: false,
            drop_media_captions: false,
            noforwards: false,
            allow_paid_floodskip: false,
            from_peer,
            id: vec![target_id],
            random_id: vec![random_id()],
            to_peer: tl::enums::InputPeer::PeerSelf,
            top_msg_id: None,
            reply_to: None,
            schedule_date: None,
            send_as: None,
            quick_reply_shortcut: None,
            allow_paid_stars: None,
            effect: None,
            video_timestamp: None,
            schedule_repeat_period: None,
            suggested_post: None,
        })
        .await?;

    crate::commands::meta::edit(msg, client, "📌 <b>Forwarded to Saved Messages</b>".into()).await
}

fn filename_for_media(media: &tl::enums::MessageMedia, msg_id: i32) -> String {
    match media {
        tl::enums::MessageMedia::Photo(_) => format!("photo_{msg_id}.jpg"),
        tl::enums::MessageMedia::Document(d) => match &d.document {
            Some(tl::enums::Document::Document(inner)) => inner
                .attributes
                .iter()
                .find_map(|a| match a {
                    tl::enums::DocumentAttribute::Filename(f) => Some(f.file_name.clone()),
                    _ => None,
                })
                .unwrap_or_else(|| format!("doc_{msg_id}.bin")),
            _ => format!("doc_{msg_id}.bin"),
        },
        _ => format!("media_{msg_id}.bin"),
    }
}
