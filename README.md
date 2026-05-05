# FeroUB

A simple personal userbot built on [ferogram](https://github.com/ankit-chaubey/ferogram). Mostly exists as a reference implementation showing what a real userbot looks like with ferogram.

## Setup

```sh
cp config.toml config.toml          # fill in api_id, api_hash, session_path
cargo run                           # dev
cargo build --release               # target/release/userbot
```

## Commands

All commands are self-only and silently ignored if sent by anyone else.

| Command | Description |
|---|---|
| `.ping` | Round-trip latency to Telegram |
| `.stats` | Uptime, messages handled, DC info |
| `.id` | User / chat / message IDs |
| `.json` | Raw TL JSON of the replied message |
| `.info` | Full info on a replied user or chat |
| `.purge` | Delete everything from replied message to current |
| `.purgeme` | Same range, but only your own messages |
| `.copy <src/msg_id> <dst>` | Forward without the forwarded header |
| `.get <src/msg_id> <dst>` | Download and reupload (breaks file reference) |
| `.fwd <from> <to> <start:end> <s>` | Forward a range with a delay between each |
| `.save` | Save replied media to disk and reply with the path |
| `.saved` | Forward replied message to Saved Messages |
| `.vta` | Convert video to MP3 audio (needs ffmpeg) |
| `.atv` | Convert audio to voice note (needs ffmpeg + libopus) |
| `.bash <cmd>` | Run a shell command and reply with output |
| `.backup` | Upload your session file to Saved Messages |
| `.afk [reason]` | AFK mode with auto-reply, clears on next outgoing message |

## Requirements

- Rust (edition 2024)
- ffmpeg in PATH for `.vta` / `.atv`
- ferogram 0.3.8+

## Notes

- Peer args accept `me`, `@username`, or numeric IDs
- `.fwd` start:end is a message ID range
- `.bash` output longer than the char limit gets uploaded as `output.txt`
