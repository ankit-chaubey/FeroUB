use anyhow::Result;
use std::process::Command;
use std::sync::Arc;
use std::time::Instant;

use ferogram::Client;
use ferogram::tl;
use ferogram::update::IncomingMessage;

use crate::state::AppState;
use crate::util::{esc, peer_from_id, random_id, read_bytes};

pub async fn bash(
    msg: &IncomingMessage,
    client: &Client,
    state: &Arc<AppState>,
    cmd: &str,
) -> Result<()> {
    if cmd.is_empty() {
        return Err(anyhow::anyhow!("no command given"));
    }

    crate::commands::meta::edit(msg, client, format!("⏳ <code>{}</code>", esc(cmd))).await?;

    let t = Instant::now();
    let output = Command::new("sh").args(["-c", cmd]).output()?;
    let elapsed = t.elapsed().as_millis();

    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    let combined = if stderr.is_empty() {
        stdout
    } else {
        format!("{stdout}\n[stderr]\n{stderr}")
    };

    let exit_icon = if output.status.success() {
        "✅"
    } else {
        "❌"
    };
    let exit_code = output.status.code().unwrap_or(-1);
    let limit = state.config.bash_char_limit;

    if combined.len() <= limit {
        crate::commands::meta::edit(msg, client, format!(
            "{exit_icon} <code>{}</code>  <i>({elapsed}ms · exit {exit_code})</i>\n<pre>{}</pre>",
            esc(cmd),
            esc(&combined),
        )).await?;
    } else {
        let bytes = combined.as_bytes().to_vec();
        let uploaded = client
            .upload_file(&bytes, "output.txt", "text/plain")
            .await?;

        let input_media = {
            let tl::enums::InputMedia::UploadedDocument(mut d) = uploaded.as_document_media()
            else {
                unreachable!()
            };
            d.force_file = true;
            d.mime_type = "text/plain".into();
            d.attributes = vec![tl::enums::DocumentAttribute::Filename(
                tl::types::DocumentAttributeFilename {
                    file_name: "output.txt".into(),
                },
            )];
            tl::enums::InputMedia::UploadedDocument(d)
        };

        let peer = peer_from_id(msg.chat_id());

        client
            .invoke(&tl::functions::messages::SendMedia {
                silent: false,
                background: false,
                clear_draft: false,
                noforwards: false,
                update_stickersets_order: false,
                invert_media: false,
                allow_paid_floodskip: false,
                peer,
                reply_to: None,
                media: input_media,
                message: format!("{exit_icon} {}  ({elapsed}ms · exit {exit_code})", esc(cmd),),
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

        msg.delete().await?;
    }

    Ok(())
}

pub async fn backup(msg: &IncomingMessage, client: &Client, state: &Arc<AppState>) -> Result<()> {
    let session_path = std::path::Path::new(&state.config.session_path);
    if !session_path.exists() {
        return Err(anyhow::anyhow!(
            "session file not found: {}",
            state.config.session_path
        ));
    }

    crate::commands::meta::edit(msg, client, "📤 Uploading session backup…".into()).await?;

    let filename = session_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("session.session")
        .to_string();

    let bytes = read_bytes(session_path)?;
    let uploaded = client
        .upload_file(&bytes, &filename, "application/octet-stream")
        .await?;

    let input_media = {
        let tl::enums::InputMedia::UploadedDocument(mut d) = uploaded.as_document_media() else {
            unreachable!()
        };
        d.force_file = true;
        d.mime_type = "application/octet-stream".into();
        d.attributes = vec![tl::enums::DocumentAttribute::Filename(
            tl::types::DocumentAttributeFilename {
                file_name: filename,
            },
        )];
        tl::enums::InputMedia::UploadedDocument(d)
    };

    client
        .invoke(&tl::functions::messages::SendMedia {
            silent: true,
            background: false,
            clear_draft: false,
            noforwards: false,
            update_stickersets_order: false,
            invert_media: false,
            allow_paid_floodskip: false,
            peer: tl::enums::InputPeer::PeerSelf,
            reply_to: None,
            media: input_media,
            message: "🔐 Session backup".into(),
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

    crate::commands::meta::edit(
        msg,
        client,
        "✅ <b>Session backed up to Saved Messages</b>".into(),
    )
    .await
}
