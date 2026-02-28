mod bridge;

use anyhow::Result;
use bridge::{BaileysBridge, BridgeEvent};
use kova_core::agent::Agent;
use kova_core::config::Config;
use kova_core::llm::LlmClient;
use std::path::PathBuf;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let base_dir = find_project_root()?;
    let config = Config::load(&base_dir.join("config/kovaclaw.json"))?;
    let identity = config.load_identity(&base_dir)?;

    let llm = LlmClient::new(config.llm);
    let mut agent = Agent::new(llm, identity);

    let bridge_dir = base_dir.join("bridge");
    let auth_dir = std::env::var("BAILEYS_AUTH_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| bridge_dir.join("auth_state"));

    println!("[kovaclaw-wa] starting bridge...");
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
            BridgeEvent::Message { jid, text, push_name, .. } => {
                let label = if push_name.is_empty() { &jid } else { &push_name };
                println!("[{label}] {text}");

                match agent.send(&text).await {
                    Ok(response) => {
                        // Strip tool calls from response for WhatsApp
                        let clean = strip_tool_calls(&response);
                        if !clean.trim().is_empty() {
                            println!("[kova -> {label}] {clean}");
                            bridge.send_message(&jid, &clean).await?;
                        }
                    }
                    Err(e) => {
                        tracing::error!("LLM error: {e}");
                        bridge.send_message(&jid, "Error processing message.").await?;
                    }
                }
            }
            BridgeEvent::Sent { jid } => {
                tracing::debug!("sent to {jid}");
            }
            BridgeEvent::Error { message } => {
                tracing::error!("bridge error: {message}");
            }
        }
    }

    bridge.kill().await?;
    Ok(())
}

fn strip_tool_calls(text: &str) -> String {
    let mut result = String::new();
    let mut remaining = text;
    while let Some(start) = remaining.find("<tool_call>") {
        result.push_str(&remaining[..start]);
        if let Some(end) = remaining[start..].find("</tool_call>") {
            remaining = &remaining[start + end + 12..];
        } else {
            break;
        }
    }
    result.push_str(remaining);
    result.trim().to_string()
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
