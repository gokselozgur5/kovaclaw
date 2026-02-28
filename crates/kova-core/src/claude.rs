use anyhow::Result;
use std::process::Stdio;
use tokio::process::Command;
use uuid::Uuid;

const CLAUDE_BIN: &str = "/Users/gok/.claude/local/claude";

const KOVACLAW_NS: Uuid = Uuid::from_bytes([
    0x6b, 0x6f, 0x76, 0x61, 0x63, 0x6c, 0x61, 0x77,
    0x2d, 0x77, 0x68, 0x61, 0x74, 0x73, 0x61, 0x70,
]);

pub struct ClaudeClient {
    system_prompt: Option<String>,
    model: Option<String>,
}

impl ClaudeClient {
    pub fn new() -> Self {
        Self {
            system_prompt: None,
            model: None,
        }
    }

    pub fn with_system_prompt(mut self, prompt: String) -> Self {
        self.system_prompt = Some(prompt);
        self
    }

    pub fn with_model(mut self, model: String) -> Self {
        self.model = Some(model);
        self
    }

    fn session_id_for(conversation_id: &str) -> String {
        Uuid::new_v5(&KOVACLAW_NS, conversation_id.as_bytes()).to_string()
    }

    pub async fn send(&self, conversation_id: &str, message: &str) -> Result<String> {
        let session_id = Self::session_id_for(conversation_id);

        let mut cmd = Command::new(CLAUDE_BIN);
        cmd.arg("-p");
        cmd.arg("--dangerously-skip-permissions");
        cmd.env_remove("CLAUDECODE");

        if let Some(ref model) = self.model {
            cmd.arg("--model").arg(model);
        }

        if let Some(ref prompt) = self.system_prompt {
            cmd.arg("--system-prompt").arg(prompt);
        }

        cmd.arg("--resume").arg(&session_id);

        cmd.stdin(Stdio::piped());
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());

        let mut child = cmd.spawn()?;

        if let Some(mut stdin) = child.stdin.take() {
            use tokio::io::AsyncWriteExt;
            stdin.write_all(message.as_bytes()).await?;
            drop(stdin);
        }

        let output = child.wait_with_output().await?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("claude CLI failed: {}", stderr);
        }

        let response = String::from_utf8_lossy(&output.stdout).trim().to_string();
        Ok(response)
    }
}
