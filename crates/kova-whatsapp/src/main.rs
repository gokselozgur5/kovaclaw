mod bridge;

use anyhow::Result;
use bridge::{BaileysBridge, BridgeEvent};
use kova_core::claude::ClaudeClient;
use kova_core::config::Config;
use std::path::PathBuf;
use std::time::Instant;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let base_dir = find_project_root()?;
    let config = Config::load(&base_dir.join("config/kovaclaw.json"))?;
    let identity = config.load_identity(&base_dir)?;

    let mut claude = ClaudeClient::new()
        .with_system_prompt(identity)
        .with_model("sonnet".to_string());

    let bridge_dir = base_dir.join("bridge");
    let auth_dir = std::env::var("BAILEYS_AUTH_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| bridge_dir.join("auth_state"));

    let mut last_self_send: Option<Instant> = None;

    println!("[kovaclaw-wa] starting bridge (claude cli backend)...");
    let (mut bridge, mut events) = BaileysBridge::spawn(&bridge_dir, &auth_dir).await?;

    while let Some(event) = events.recv().await {
        match event {
            BridgeEvent::Connected => {
                println!("[kovaclaw-wa] connected to WhatsApp");
            }
            BridgeEvent::Disconnected { reason } => {
                println!("[kovaclaw-wa] disconnected: {reason}");
                break;
            }
            BridgeEvent::Message { jid, text, push_name, from_me, .. } => {
                let label = if push_name.is_empty() { &jid } else { &push_name };

                if from_me {
                    if let Some(t) = last_self_send {
                        if t.elapsed().as_secs() < 30 {
                            println!("[kova echo, skipped]");
                            continue;
                        }
                    }
                    println!("[self] {text}");
                } else {
                    println!("[{label}] {text}");
                }

                match claude.send(&jid, &text).await {
                    Ok(response) => {
                        if !response.trim().is_empty() {
                            let response = if response.len() > 4000 {
                                format!("{}...\n[truncated]", &response[..4000])
                            } else {
                                response
                            };
                            println!("[kova -> {label}] {response}");
                            last_self_send = Some(Instant::now());
                            bridge.send_message(&jid, &response).await?;
                        }
                    }
                    Err(e) => {
                        tracing::error!("claude error: {e}");
                        last_self_send = Some(Instant::now());
                        bridge.send_message(&jid, &format!("Error: {e}")).await?;
                    }
                }
            }
            BridgeEvent::Sent { jid } => {
                tracing::debug!("sent to {jid}");
            }
            BridgeEvent::Qr { .. } => {
                println!("[kovaclaw-wa] QR code generated (check terminal)");
            }
            BridgeEvent::Error { message } => {
                tracing::error!("bridge error: {message}");
            }
        }
    }

    bridge.kill().await?;
    Ok(())
}

fn find_project_root() -> Result<PathBuf> {
    if let Ok(root) = std::env::var("KOVACLAW_ROOT") {
        return Ok(PathBuf::from(root));
    }
    let mut dir = std::env::current_dir()?;
    loop {
        if dir.join("config/kovaclaw.json").exists() {
            return Ok(dir);
        }
        if !dir.pop() { break; }
    }
    anyhow::bail!("kovaclaw.json not found. Set KOVACLAW_ROOT or run from the kovaclaw/ directory.")
}
