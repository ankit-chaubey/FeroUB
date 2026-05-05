use anyhow::{Result, bail};
use std::sync::Arc;
use std::time::Duration;

use ferogram::Client;
use ferogram::tl;
use ferogram::update::IncomingMessage;

use crate::state::AppState;
use crate::util::{
    media_to_location, messages_from_response, msg_id_from_enum, parse_src_arg, peer_from_id,
    random_id, read_bytes, resolve_peer,
};

pub async fn purge(
    msg: &IncomingMessage,
    client: &Client,
    state: &Arc<AppState>,
    self_only: bool,
) -> Result<()> {
    let start_id = msg
        .reply_to_message_id()
        .ok_or_else(|| anyhow::anyhow!("reply to the first message you want to delete"))?;
    let end_id = msg.id();
    let peer = peer_from_id(msg.chat_id());

    let mut to_delete: Vec<i32> = Vec::new();
    let mut offset_id = end_id + 1;

    loop {
        let batch_limit = (offset_id - start_id).min(100) as i32;
        if batch_limit <= 0 {
            break;
        }

        let resp = client
            .invoke(&tl::functions::messages::GetHistory {
                peer: peer.clone(),
                offset_id,
                offset_date: 0,
                add_offset: 0,
                limit: batch_limit,
                max_id: end_id,
                min_id: start_id - 1,
                hash: 0,
            })
            .await?;

        let msgs = messages_from_response(resp);
        if msgs.is_empty() {
            break;
        }

        for m in &msgs {
            if let Some(id) = msg_id_from_enum(m) {
                let include = if self_only {
                    match m {
                        tl::enums::Message::Message(mm) => matches!(
                            &mm.from_id,
                            Some(tl::enums::Peer::User(u)) if u.user_id == state.self_id
                        ),
                        _ => false,
                    }
                } else {
                    true
                };
                if include {
                    to_delete.push(id);
                }
            }
        }

        let oldest = msgs.last().and_then(msg_id_from_enum).unwrap_or(0);
        if oldest <= start_id {
            break;
        }
        offset_id = oldest;
    }

    if to_delete.is_empty() {
        return Ok(());
    }

    for chunk in to_delete.chunks(100) {
        // revoke = true to delete for everyone (where allowed)
        client.delete_messages(chunk, true).await?;
    }

    Ok(())
}

pub async fn copy(msg: &IncomingMessage, client: &Client, args: &str) -> Result<()> {
    let (src_chat, src_msg_id, dst_chat) = parse_copy_args(args)?;

    let from_peer = resolve_peer(src_chat, client).await?;
    let to_peer = resolve_peer(dst_chat, client).await?;

    client
        .invoke(&tl::functions::messages::ForwardMessages {
            silent: false,
            background: false,
            with_my_score: false,
            drop_author: true,
            drop_media_captions: false,
            noforwards: false,
            allow_paid_floodskip: false,
            from_peer,
            id: vec![src_msg_id],
            random_id: vec![random_id()],
            to_peer,
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

    msg.delete().await?;
    Ok(())
}

pub async fn get(msg: &IncomingMessage, client: &Client, args: &str) -> Result<()> {
    let (src_chat, src_msg_id, dst_chat) = parse_copy_args(args)?;

    let _from_peer = resolve_peer(src_chat, client).await?;
    let to_peer = resolve_peer(dst_chat, client).await?;

    let resp = client
        .invoke(&tl::functions::messages::GetMessages {
            id: vec![tl::enums::InputMessage::Id(tl::types::InputMessageId {
                id: src_msg_id,
            })],
        })
        .await?;

    let msgs = messages_from_response(resp);
    let source = msgs
        .into_iter()
        .find_map(|m| match m {
            tl::enums::Message::Message(m) => Some(m),
            _ => None,
        })
        .ok_or_else(|| anyhow::anyhow!("source message not found"))?;

    let caption = source.message.clone();

    if let Some(media) = &source.media {
        let location = media_to_location(media)?;
        let tmp_dir = tempfile::tempdir()?;
        let tmp_path = tmp_dir.path().join("media_dl");

        client.download_file(location, &tmp_path).await?;

        let bytes = read_bytes(&tmp_path)?;
        let uploaded = client
            .upload_file(&bytes, "media", "application/octet-stream")
            .await?;

        let input_media = {
            let tl::enums::InputMedia::UploadedDocument(mut d) = uploaded.as_document_media()
            else {
                unreachable!()
            };
            d.mime_type = "application/octet-stream".into();
            d.attributes = vec![];
            tl::enums::InputMedia::UploadedDocument(d)
        };

        client
            .invoke(&tl::functions::messages::SendMedia {
                silent: false,
                background: false,
                clear_draft: false,
                noforwards: false,
                update_stickersets_order: false,
                invert_media: false,
                allow_paid_floodskip: false,
                peer: to_peer,
                reply_to: None,
                media: input_media,
                message: caption,
                random_id: random_id(),
                reply_markup: None,
                entities: None,
                schedule_date: None,
                send_as: None,
                quick_reply_shortcut: None,
                effect: None,
                allow_paid_stars: None,
                schedule_repeat_period: None,
                suggested_post: None,
            })
            .await?;
    } else {
        // Send plain text directly via raw TL (InputPeer doesn't implement Into<PeerRef>)
        client
            .invoke(&tl::functions::messages::SendMessage {
                no_webpage: false,
                silent: false,
                background: false,
                clear_draft: false,
                noforwards: false,
                update_stickersets_order: false,
                invert_media: false,
                allow_paid_floodskip: false,
                peer: to_peer,
                reply_to: None,
                message: source.message.clone(),
                random_id: random_id(),
                reply_markup: None,
                entities: None,
                schedule_date: None,
                send_as: None,
                quick_reply_shortcut: None,
                effect: None,
                allow_paid_stars: None,
                schedule_repeat_period: None,
                suggested_post: None,
            })
            .await?;
    }

    msg.delete().await?;
    Ok(())
}

pub async fn fwd(msg: &IncomingMessage, client: &Client, args: &str) -> Result<()> {
    let parts: Vec<&str> = args.splitn(4, ' ').collect();
    if parts.len() < 4 {
        bail!("usage: .fwd <from_chat> <to_chat> <start_id:end_id> <delay_secs>");
    }

    let from_chat = parts[0];
    let to_chat = parts[1];
    let range_str = parts[2];
    let delay_secs: u64 = parts[3]
        .parse()
        .map_err(|_| anyhow::anyhow!("delay must be a number in seconds"))?;

    let (start_id, end_id) = parse_range(range_str)?;
    let from_peer = resolve_peer(from_chat, client).await?;
    let to_peer = resolve_peer(to_chat, client).await?;

    let total = (end_id - start_id + 1).max(0) as usize;

    crate::commands::meta::edit(
        msg,
        client,
        format!("⏩ forwarding <code>{total}</code> messages…"),
    )
    .await?;

    let mut forwarded = 0usize;

    for id in start_id..=end_id {
        let result = client
            .invoke(&tl::functions::messages::ForwardMessages {
                silent: false,
                background: false,
                with_my_score: false,
                drop_author: false,
                drop_media_captions: false,
                noforwards: false,
                allow_paid_floodskip: false,
                from_peer: from_peer.clone(),
                id: vec![id],
                random_id: vec![random_id()],
                to_peer: to_peer.clone(),
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
            .await;

        if result.is_ok() {
            forwarded += 1;
        }

        if delay_secs > 0 {
            tokio::time::sleep(Duration::from_secs(delay_secs)).await;
        }
    }

    crate::commands::meta::edit(
        msg,
        client,
        format!("✅ forwarded <code>{forwarded}/{total}</code> messages"),
    )
    .await?;

    Ok(())
}

fn parse_copy_args(args: &str) -> Result<(&str, i32, &str)> {
    let mut parts = args.splitn(2, ' ');
    let src = parts
        .next()
        .ok_or_else(|| anyhow::anyhow!("missing source"))?;
    let dst = parts
        .next()
        .ok_or_else(|| anyhow::anyhow!("missing destination"))?
        .trim();
    let (chat, id) = parse_src_arg(src)?;
    Ok((chat, id, dst))
}

fn parse_range(s: &str) -> Result<(i32, i32)> {
    let (a, b) = s
        .split_once(':')
        .ok_or_else(|| anyhow::anyhow!("range must be start:end"))?;
    let start: i32 = a.parse()?;
    let end: i32 = b.parse()?;
    if start > end {
        bail!("start must be ≤ end");
    }
    Ok((start, end))
}
