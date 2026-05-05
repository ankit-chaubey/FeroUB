use anyhow::Result;
use std::process::Command;

use ferogram::Client;
use ferogram::tl;
use ferogram::update::IncomingMessage;

use crate::util::{media_to_location, random_id, read_bytes};

pub async fn vta(msg: &IncomingMessage, client: &Client) -> Result<()> {
    let media = get_replied_media(msg, client).await?;

    crate::commands::meta::edit(msg, client, "🎵 Converting video → audio…".into()).await?;

    let tmp_dir = tempfile::tempdir()?;
    let in_path = tmp_dir.path().join("input");
    let out_path = tmp_dir.path().join("output.mp3");

    let location = media_to_location(&media)?;
    client.download_file(location, &in_path).await?;

    ffmpeg_run(&[
        "-y",
        "-i",
        in_path.to_str().unwrap(),
        "-vn",
        "-acodec",
        "libmp3lame",
        "-q:a",
        "2",
        out_path.to_str().unwrap(),
    ])?;

    let bytes = read_bytes(&out_path)?;
    let uploaded = client
        .upload_file(&bytes, "audio.mp3", "audio/mpeg")
        .await?;

    let input_media = {
        let tl::enums::InputMedia::UploadedDocument(mut d) = uploaded.as_document_media() else {
            unreachable!()
        };
        d.mime_type = "audio/mpeg".into();
        d.attributes = vec![tl::enums::DocumentAttribute::Audio(
            tl::types::DocumentAttributeAudio {
                voice: false,
                duration: 0,
                title: None,
                performer: None,
                waveform: None,
            },
        )];
        tl::enums::InputMedia::UploadedDocument(d)
    };

    let peer = crate::util::peer_from_id(msg.chat_id());

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
            message: String::new(),
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
    Ok(())
}

pub async fn atv(msg: &IncomingMessage, client: &Client) -> Result<()> {
    let media = get_replied_media(msg, client).await?;

    crate::commands::meta::edit(msg, client, "🎙 Converting audio → voice note…".into()).await?;

    let tmp_dir = tempfile::tempdir()?;
    let in_path = tmp_dir.path().join("input");
    let out_path = tmp_dir.path().join("output.ogg");

    let location = media_to_location(&media)?;
    client.download_file(location, &in_path).await?;

    ffmpeg_run(&[
        "-y",
        "-i",
        in_path.to_str().unwrap(),
        "-c:a",
        "libopus",
        "-b:a",
        "64k",
        "-vbr",
        "on",
        "-ar",
        "48000",
        out_path.to_str().unwrap(),
    ])?;

    let bytes = read_bytes(&out_path)?;
    let uploaded = client.upload_file(&bytes, "voice.ogg", "audio/ogg").await?;

    let input_media = {
        let tl::enums::InputMedia::UploadedDocument(mut d) = uploaded.as_document_media() else {
            unreachable!()
        };
        d.mime_type = "audio/ogg".into();
        d.attributes = vec![tl::enums::DocumentAttribute::Audio(
            tl::types::DocumentAttributeAudio {
                duration: 0,
                title: None,
                performer: None,
                waveform: None,
            },
        )];
        tl::enums::InputMedia::UploadedDocument(d)
    };

    let peer = crate::util::peer_from_id(msg.chat_id());

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
            message: String::new(),
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
    Ok(())
}

async fn get_replied_media(
    msg: &IncomingMessage,
    client: &Client,
) -> Result<tl::enums::MessageMedia> {
    let target_id = msg
        .reply_to_message_id()
        .ok_or_else(|| anyhow::anyhow!("reply to a video or audio message"))?;

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
        .ok_or_else(|| anyhow::anyhow!("source message not found"))?;

    source
        .media
        .ok_or_else(|| anyhow::anyhow!("replied message has no media"))
}

fn ffmpeg_run(args: &[&str]) -> Result<()> {
    let out = Command::new("ffmpeg").args(args).output()?;
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        anyhow::bail!(
            "ffmpeg failed: {}",
            &stderr[stderr.len().saturating_sub(400)..]
        );
    }
    Ok(())
}
