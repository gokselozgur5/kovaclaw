use crate::event::Message;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

#[derive(Serialize, Deserialize)]
struct SessionEntry {
    timestamp: chrono::DateTime<chrono::Utc>,
    message: Message,
}

pub struct Session {
    path: PathBuf,
}

impl Session {
    pub fn new(session_dir: &Path, id: &str) -> Result<Self> {
        fs::create_dir_all(session_dir)?;
        Ok(Self {
            path: session_dir.join(format!("{id}.jsonl")),
        })
    }

    pub fn append(&self, message: &Message) -> Result<()> {
        let entry = SessionEntry {
            timestamp: chrono::Utc::now(),
            message: message.clone(),
        };
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)?;
        let line = serde_json::to_string(&entry)?;
        writeln!(file, "{line}")?;
        Ok(())
    }

    pub fn load(&self) -> Result<Vec<Message>> {
        if !self.path.exists() {
            return Ok(Vec::new());
        }
        let content = fs::read_to_string(&self.path)?;
        let mut messages = Vec::new();
        for line in content.lines() {
            if let Ok(entry) = serde_json::from_str::<SessionEntry>(line) {
                messages.push(entry.message);
            }
        }
        Ok(messages)
    }
}
