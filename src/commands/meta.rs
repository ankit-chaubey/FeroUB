use anyhow::Result;
use std::sync::Arc;
use std::time::Instant;

use crate::state::AppState;
use crate::util::{esc, fmt_duration};
use ferogram::tl;
use ferogram::update::IncomingMessage;
use ferogram::{Client, InputMessage};

pub async fn ping(msg: &IncomingMessage, client: &Client) -> Result<()> {
    let t = Instant::now();
    client.invoke(&tl::functions::help::GetNearestDc {}).await?;
    let ms = t.elapsed().as_millis();
    edit(msg, client, format!("🏓 <b>Pong!</b>  <code>{ms}ms</code>")).await
}

pub async fn stats(msg: &IncomingMessage, client: &Client, state: &Arc<AppState>) -> Result<()> {
    let uptime = fmt_duration(state.started.elapsed().as_secs());
    let handled = state.msg_count.load(std::sync::atomic::Ordering::Relaxed);

    let dc_line = match client.invoke(&tl::functions::help::GetNearestDc {}).await {
        Ok(tl::enums::NearestDc::NearestDc(dc)) => format!(
            "├ <b>This DC:</b>  <code>{}</code>\n└ <b>Country:</b>  <code>{}</code>",
            dc.this_dc, dc.country,
        ),
        Err(_) => "└ <b>DC info:</b>  <code>unavailable</code>".into(),
    };

    edit(
        msg,
        client,
        format!(
            "📊 <b>Stats</b>\n\
         ├ <b>Uptime:</b>   <code>{uptime}</code>\n\
         ├ <b>Handled:</b>  <code>{handled}</code> updates\n\
         {dc_line}",
        ),
    )
    .await
}

pub async fn id(msg: &IncomingMessage) -> Result<()> {
    let chat_id = msg.chat_id();
    let msg_id = msg.id();

    let mut text = format!(
        "🪪 <b>IDs</b>\n\
         ├ <b>Chat:</b>  <code>{chat_id}</code>\n\
         ├ <b>Msg:</b>   <code>{msg_id}</code>\n"
    );

    if let Some(sid) = msg.sender_user_id() {
        text.push_str(&format!("├ <b>From:</b>  <code>{sid}</code>\n"));
    }
    if let Some(rid) = msg.reply_to_message_id() {
        text.push_str(&format!("└ <b>Reply to:</b>  <code>{rid}</code>\n"));
    }

    msg.respond(InputMessage::html(text)).await?;
    msg.delete().await?;
    Ok(())
}

pub async fn json(msg: &IncomingMessage, client: &Client) -> Result<()> {
    let target_id = msg
        .reply_to_message_id()
        .ok_or_else(|| anyhow::anyhow!("reply to a message first"))?;

    let resp = client
        .invoke(&tl::functions::messages::GetMessages {
            id: vec![tl::enums::InputMessage::Id(tl::types::InputMessageId {
                id: target_id,
            })],
        })
        .await?;

    let messages = crate::util::messages_from_response(resp);
    let first = messages
        .first()
        .ok_or_else(|| anyhow::anyhow!("message not found"))?;

    let json_str = format!("{:#?}", first);

    let html = format!(
        "📄 <b>Message JSON</b>\n<pre><code class=\"language-json\">{}</code></pre>",
        esc(&json_str)
    );

    if html.len() <= 4096 {
        edit(msg, client, html).await?;
    } else {
        let tmp = tempfile::NamedTempFile::new()?;
        std::fs::write(tmp.path(), &json_str)?;
        let bytes = crate::util::read_bytes(tmp.path())?;
        let uploaded = client
            .upload_file(&bytes, &format!("msg_{target_id}.json"), "application/json")
            .await?;

        let input_media = {
            let tl::enums::InputMedia::UploadedDocument(mut d) = uploaded.as_document_media()
            else {
                unreachable!()
            };
            d.force_file = true;
            d.mime_type = "application/json".into();
            d.attributes = vec![tl::enums::DocumentAttribute::Filename(
                tl::types::DocumentAttributeFilename {
                    file_name: format!("msg_{target_id}.json"),
                },
            )];
            tl::enums::InputMedia::UploadedDocument(d)
        };

        msg.respond(
            InputMessage::html(format!("📄 message <code>{target_id}</code> JSON"))
                .copy_media(input_media),
        )
        .await?;
        msg.delete().await?;
    }

    Ok(())
}

pub async fn info(msg: &IncomingMessage, client: &Client) -> Result<()> {
    let target_id = msg
        .reply_to_message_id()
        .ok_or_else(|| anyhow::anyhow!("reply to a message to get info"))?;

    let chat_id = msg.chat_id();
    let chat_peer = crate::util::peer_from_id(chat_id);

    let input_user = tl::enums::InputUser::FromMessage(tl::types::InputUserFromMessage {
        peer: chat_peer.clone(),
        msg_id: target_id,
        user_id: msg.sender_user_id().unwrap_or(0),
    });

    let full_resp = client
        .invoke(&tl::functions::users::GetFullUser { id: input_user })
        .await;

    let text = match full_resp {
        Ok(tl::enums::users::UserFull::UserFull(uf)) => {
            let u = uf
                .users
                .into_iter()
                .find_map(|u| match u {
                    tl::enums::User::User(u) => Some(u),
                    _ => None,
                })
                .ok_or_else(|| anyhow::anyhow!("user not in response"))?;

            let name = format!(
                "{} {}",
                u.first_name.as_deref().unwrap_or(""),
                u.last_name.as_deref().unwrap_or(""),
            )
            .trim()
            .to_string();

            let username = u
                .username
                .as_deref()
                .map(|s| format!("@{s}"))
                .unwrap_or_else(|| "-".into());

            let tl::enums::UserFull::UserFull(full_user) = uf.full_user;
            let bio = full_user.about.as_deref().unwrap_or("-");

            format!(
                "👤 <b>User Info</b>\n\
                 ├ <b>Name:</b>     {}\n\
                 ├ <b>Username:</b> {}\n\
                 ├ <b>ID:</b>       <code>{}</code>\n\
                 ├ <b>Bot:</b>      {}\n\
                 ├ <b>Premium:</b>  {}\n\
                 └ <b>Bio:</b>      {}",
                esc(&name),
                esc(&username),
                u.id,
                if u.bot { "yes" } else { "no" },
                if u.premium { "yes" } else { "no" },
                esc(bio),
            )
        }
        Err(_) => {
            let input_ch =
                tl::enums::InputChannel::FromMessage(tl::types::InputChannelFromMessage {
                    peer: chat_peer,
                    msg_id: target_id,
                    channel_id: -chat_id - 1_000_000_000,
                });

            let full = client
                .invoke(&tl::functions::channels::GetFullChannel { channel: input_ch })
                .await?;

            let tl::enums::messages::ChatFull::ChatFull(cf) = full;

            let ch = cf.chats.into_iter().find_map(|c| match c {
                tl::enums::Chat::Channel(c) => Some(c),
                _ => None,
            });

            match ch {
                Some(c) => match cf.full_chat {
                    tl::enums::ChatFull::ChannelFull(fc) => format!(
                        "📢 <b>Channel/Group Info</b>\n\
                         ├ <b>Title:</b>   {}\n\
                         ├ <b>ID:</b>      <code>{}</code>\n\
                         ├ <b>Members:</b> <code>{}</code>\n\
                         └ <b>About:</b>   {}",
                        esc(&c.title),
                        c.id,
                        fc.participants_count.unwrap_or(0),
                        esc(&fc.about),
                    ),
                    _ => "Could not resolve channel info.".into(),
                },
                None => "Could not resolve peer info.".into(),
            }
        }
    };

    edit(msg, client, text).await
}

pub async fn edit(msg: &IncomingMessage, client: &Client, html: String) -> Result<()> {
    let im = InputMessage::html(&html);
    let raw_peer = msg
        .peer_id()
        .ok_or_else(|| anyhow::anyhow!("message has no peer"))?;
    let peer = client
        .resolve_to_input_peer(raw_peer)
        .await
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    client
        .invoke(&tl::functions::messages::EditMessage {
            no_webpage: false,
            invert_media: false,
            peer,
            id: msg.id(),
            message: Some(im.text),
            media: None,
            reply_markup: None,
            entities: im.entities,
            schedule_date: None,
            schedule_repeat_period: None,
            quick_reply_shortcut_id: None,
        })
        .await?;
    Ok(())
}
