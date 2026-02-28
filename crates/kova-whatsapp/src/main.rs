mod bridge;

use anyhow::Result;
use bridge::{BaileysBridge, BridgeEvent};
use kova_core::agent::Agent;
use kova_core::config::Config;
use kova_core::llm::LlmClient;
use std::collections::HashSet;
use std::path::PathBuf;
use std::time::Instant;

// Tools that auto-approve without user confirmation on WhatsApp
const WA_AUTO_APPROVE: &[&str] = &["read_file", "shell_exec"];

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let base_dir = find_project_root()?;
    let config = Config::load(&base_dir.join("config/kovaclaw.json"))?;
    let identity = config.load_identity(&base_dir)?;

    let llm = LlmClient::new(config.llm);
    let mut agent = Agent::new(llm, identity);

    let whitelist: HashSet<String> = WA_AUTO_APPROVE.iter().map(|s| s.to_string()).collect();

    let bridge_dir = base_dir.join("bridge");
    let auth_dir = std::env::var("BAILEYS_AUTH_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| bridge_dir.join("auth_state"));

    let mut last_self_send: Option<Instant> = None;

    println!("[kovaclaw-wa] starting bridge...");
    println!("[kovaclaw-wa] auto-approve tools: {:?}", WA_AUTO_APPROVE);
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
                    // Skip kova's own messages (echoed back within 30s of sending)
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

                let wl = whitelist.clone();
                match agent.run_loop(&text, |name| wl.contains(name)).await {
                    Ok(result) => {
                        for exec in &result.tool_log {
                            let status = if exec.success { "ok" } else { "fail" };
                            let preview = if exec.output.len() > 100 {
                                format!("{}...", &exec.output[..100])
                            } else {
                                exec.output.clone()
                            };
                            println!("  [tool:{} -> {status}] {preview}", exec.name);
                        }

                        if !result.final_text.trim().is_empty() {
                            let text = if result.final_text.len() > 4000 {
                                format!("{}...\n[truncated]", &result.final_text[..4000])
                            } else {
                                result.final_text
                            };
                            println!("[kova -> {label}] {text}");
                            last_self_send = Some(Instant::now());
                            bridge.send_message(&jid, &text).await?;
                        }
                    }
                    Err(e) => {
                        tracing::error!("agent error: {e}");
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
