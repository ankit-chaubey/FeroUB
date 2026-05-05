use anyhow::{Result, bail};
use ferogram::Client;
use ferogram::tl;

pub fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

pub fn fmt_duration(secs: u64) -> String {
    let d = secs / 86400;
    let h = (secs % 86400) / 3600;
    let m = (secs % 3600) / 60;
    let s = secs % 60;
    match (d, h, m) {
        (0, 0, 0) => format!("{}s", s),
        (0, 0, _) => format!("{}m {}s", m, s),
        (0, _, _) => format!("{}h {}m", h, m),
        _ => format!("{}d {}h {}m", d, h, m),
    }
}

pub fn peer_from_id(id: i64) -> tl::enums::InputPeer {
    if id > 0 {
        tl::enums::InputPeer::User(tl::types::InputPeerUser {
            user_id: id,
            access_hash: 0,
        })
    } else if id > -1_000_000_000 {
        tl::enums::InputPeer::Chat(tl::types::InputPeerChat { chat_id: -id })
    } else {
        tl::enums::InputPeer::Channel(tl::types::InputPeerChannel {
            channel_id: -id - 1_000_000_000,
            access_hash: 0,
        })
    }
}

pub async fn resolve_peer(s: &str, client: &Client) -> Result<tl::enums::InputPeer> {
    let s = s.trim();

    if s == "me" || s == "self" {
        return Ok(tl::enums::InputPeer::PeerSelf);
    }

    if let Ok(id) = s.parse::<i64>() {
        return Ok(peer_from_id(id));
    }

    let username = s.trim_start_matches('@');
    let r = client
        .invoke(&tl::functions::contacts::ResolveUsername {
            referer: None,
            username: username.to_string(),
        })
        .await?;

    let tl::enums::contacts::ResolvedPeer::ResolvedPeer(rp) = r;

    match rp.peer {
        tl::enums::Peer::User(u) => {
            let user = rp
                .users
                .into_iter()
                .find_map(|x| match x {
                    tl::enums::User::User(u2) if u2.id == u.user_id => Some(u2),
                    _ => None,
                })
                .ok_or_else(|| anyhow::anyhow!("user not in resolve response"))?;
            Ok(tl::enums::InputPeer::User(tl::types::InputPeerUser {
                user_id: user.id,
                access_hash: user.access_hash.unwrap_or(0),
            }))
        }
        tl::enums::Peer::Channel(c) => {
            let ch = rp
                .chats
                .into_iter()
                .find_map(|x| match x {
                    tl::enums::Chat::Channel(c2) if c2.id == c.channel_id => Some(c2),
                    _ => None,
                })
                .ok_or_else(|| anyhow::anyhow!("channel not in resolve response"))?;
            Ok(tl::enums::InputPeer::Channel(tl::types::InputPeerChannel {
                channel_id: ch.id,
                access_hash: ch.access_hash.unwrap_or(0),
            }))
        }
        tl::enums::Peer::Chat(c) => Ok(tl::enums::InputPeer::Chat(tl::types::InputPeerChat {
            chat_id: c.chat_id,
        })),
    }
}

pub fn parse_src_arg(arg: &str) -> Result<(&str, i32)> {
    let (chat, id) = arg
        .rsplit_once('/')
        .ok_or_else(|| anyhow::anyhow!("expected <chat/msg_id>"))?;
    Ok((chat, id.parse()?))
}

pub fn random_id() -> i64 {
    use rand::Rng as _;
    rand::thread_rng().r#gen()
}

pub fn messages_from_response(resp: tl::enums::messages::Messages) -> Vec<tl::enums::Message> {
    match resp {
        tl::enums::messages::Messages::Messages(m) => m.messages,
        tl::enums::messages::Messages::Slice(m) => m.messages,
        tl::enums::messages::Messages::ChannelMessages(m) => m.messages,
        tl::enums::messages::Messages::NotModified(_) => vec![],
    }
}

pub fn msg_id_from_enum(m: &tl::enums::Message) -> Option<i32> {
    match m {
        tl::enums::Message::Message(m) => Some(m.id),
        tl::enums::Message::Service(m) => Some(m.id),
        tl::enums::Message::Empty(m) => Some(m.id),
    }
}

/// Extract an InputFileLocation from a MessageMedia (for download_media_to_file).
pub fn media_to_location(media: &tl::enums::MessageMedia) -> Result<tl::enums::InputFileLocation> {
    match media {
        tl::enums::MessageMedia::Document(d) => match &d.document {
            Some(tl::enums::Document::Document(doc)) => {
                Ok(tl::enums::InputFileLocation::InputDocumentFileLocation(
                    tl::types::InputDocumentFileLocation {
                        id: doc.id,
                        access_hash: doc.access_hash,
                        file_reference: doc.file_reference.clone(),
                        thumb_size: String::new(),
                    },
                ))
            }
            _ => bail!("no document in media"),
        },
        tl::enums::MessageMedia::Photo(p) => match &p.photo {
            Some(tl::enums::Photo::Photo(photo)) => {
                let thumb = photo
                    .sizes
                    .iter()
                    .rev()
                    .find_map(|s| match s {
                        tl::enums::PhotoSize::PhotoSize(s) => Some(s.r#type.clone()),
                        _ => None,
                    })
                    .unwrap_or_else(|| "s".to_string());
                Ok(tl::enums::InputFileLocation::InputPhotoFileLocation(
                    tl::types::InputPhotoFileLocation {
                        id: photo.id,
                        access_hash: photo.access_hash,
                        file_reference: photo.file_reference.clone(),
                        thumb_size: thumb,
                    },
                ))
            }
            _ => bail!("no photo in media"),
        },
        _ => bail!("unsupported media type for download"),
    }
}

/// Read a file to bytes for upload_file.
pub fn read_bytes(path: &std::path::Path) -> Result<Vec<u8>> {
    Ok(std::fs::read(path)?)
}
