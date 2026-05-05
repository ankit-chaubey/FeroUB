mod commands;
mod config;
mod state;
mod util;

use std::io::{self, BufRead, Write};
use std::sync::Arc;

use ferogram::Client;
use ferogram::SqliteBackend;
use ferogram::Update;
use ferogram::tl;
use state::AppState;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // RUST_LOG=debug for full output, RUST_LOG=ferogram=debug,userbot=info etc.
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "ferogram=debug,userbot=debug".parse().unwrap()),
        )
        .init();

    let cfg = config::load()?;

    println!("Connecting…");

    let (client, _shutdown) = Client::builder()
        .api_id(cfg.api_id)
        .api_hash(&cfg.api_hash)
        .session_backend(Arc::new(SqliteBackend::open(&cfg.session_path)?))
        .connect()
        .await?;

    if !client.is_authorized().await? {
        sign_in(&client).await?;
        client.save_session().await?;
        println!("Signed in and session saved.");
    }

    let users = client
        .invoke(&tl::functions::users::GetUsers {
            id: vec![tl::enums::InputUser::UserSelf],
        })
        .await?;

    let self_id = users
        .into_iter()
        .find_map(|u| match u {
            tl::enums::User::User(u) => Some(u.id),
            _ => None,
        })
        .ok_or_else(|| anyhow::anyhow!("could not resolve own user"))?;

    println!("Logged in as user id {self_id}");

    let state = Arc::new(AppState::new(self_id, cfg));
    let mut stream = client.stream_updates();

    println!("Listening for updates…");

    while let Some(update) = stream.next().await {
        match update {
            Update::NewMessage(msg) => {
                state.inc_msg();

                let from_self = msg.sender_user_id() == Some(state.self_id);

                if from_self {
                    if state.is_afk().await {
                        state.set_afk(None).await;
                    }

                    if msg
                        .text()
                        .unwrap_or_default()
                        .starts_with(state.config.prefix.as_str())
                    {
                        let state = state.clone();
                        let client = client.clone();
                        tokio::spawn(async move {
                            commands::dispatch(msg, &client, &state).await;
                        });
                    }
                } else {
                    commands::afk::handle_incoming(&msg, &client, &state).await;
                }
            }
            _ => {}
        }
    }

    Ok(())
}

async fn sign_in(client: &Client) -> anyhow::Result<()> {
    let mut stdout = io::stdout();

    print!("Phone number (with country code, e.g. +91xxxxxxxxxx): ");
    stdout.flush()?;
    let phone = read_line()?;

    let token = client.request_login_code(&phone).await?;

    print!("Code sent to Telegram. Enter code: ");
    stdout.flush()?;
    let code = read_line()?;

    match client.sign_in(&token, &code).await {
        Ok(_) => {}
        Err(ferogram::SignInError::PasswordRequired(pw_token)) => {
            print!("2FA password: ");
            stdout.flush()?;
            let password = read_line()?;
            client.check_password(*pw_token, &password).await?;
        }
        Err(e) => return Err(e.into()),
    }

    Ok(())
}

fn read_line() -> anyhow::Result<String> {
    let line = io::stdin()
        .lock()
        .lines()
        .next()
        .ok_or_else(|| anyhow::anyhow!("unexpected EOF"))??;
    Ok(line.trim().to_string())
}
