use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::mpsc;

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type")]
pub enum BridgeEvent {
    #[serde(rename = "connected")]
    Connected,
    #[serde(rename = "disconnected")]
    Disconnected { reason: String },
    #[serde(rename = "message")]
    Message {
        jid: String,
        text: String,
        #[serde(default)]
        push_name: String,
        #[serde(default)]
        message_id: String,
    },
    #[serde(rename = "sent")]
    Sent { jid: String },
    #[serde(rename = "error")]
    Error { message: String },
}

#[derive(Debug, Serialize)]
struct BridgeCommand {
    r#type: String,
    jid: String,
    text: String,
}

pub struct BaileysBridge {
    child: Child,
    stdin_tx: mpsc::Sender<String>,
}

impl BaileysBridge {
    pub async fn spawn(
        bridge_dir: &Path,
        auth_dir: &Path,
    ) -> Result<(Self, mpsc::Receiver<BridgeEvent>)> {
        let mut child = Command::new("node")
            .arg("baileys_bridge.js")
            .current_dir(bridge_dir)
            .env("BAILEYS_AUTH_DIR", auth_dir)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()?;

        let stdout = child.stdout.take().unwrap();
        let stdin = child.stdin.take().unwrap();

        let (event_tx, event_rx) = mpsc::channel::<BridgeEvent>(100);
        let (stdin_tx, mut stdin_rx) = mpsc::channel::<String>(100);

        // Read stdout -> events
        tokio::spawn(async move {
            let reader = BufReader::new(stdout);
            let mut lines = reader.lines();
            while let Ok(Some(line)) = lines.next_line().await {
                match serde_json::from_str::<BridgeEvent>(&line) {
                    Ok(event) => { let _ = event_tx.send(event).await; }
                    Err(e) => tracing::warn!("bridge parse error: {e}: {line}"),
                }
            }
        });

        // Write stdin <- commands
        tokio::spawn(async move {
            let mut stdin = stdin;
            while let Some(line) = stdin_rx.recv().await {
                let _ = stdin.write_all(line.as_bytes()).await;
                let _ = stdin.write_all(b"\n").await;
                let _ = stdin.flush().await;
            }
        });

        Ok((Self { child, stdin_tx }, event_rx))
    }

    pub async fn send_message(&self, jid: &str, text: &str) -> Result<()> {
        let cmd = BridgeCommand {
            r#type: "send".into(),
            jid: jid.into(),
            text: text.into(),
        };
        let json = serde_json::to_string(&cmd)?;
        self.stdin_tx.send(json).await?;
        Ok(())
    }

    pub async fn kill(&mut self) -> Result<()> {
        self.child.kill().await?;
        Ok(())
    }
}
