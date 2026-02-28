use super::{Tool, ToolDef, ToolOutput};
use anyhow::Result;
use serde_json::json;
use std::process::Command;

pub struct ShellExec;

impl Tool for ShellExec {
    fn definition(&self) -> ToolDef {
        ToolDef {
            name: "shell_exec".into(),
            description: "Execute a shell command and return its output".into(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "command": { "type": "string", "description": "Shell command to execute" }
                },
                "required": ["command"]
            }),
        }
    }

    fn execute(&self, args: serde_json::Value) -> Result<ToolOutput> {
        let cmd = args["command"].as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing 'command' argument"))?;

        let output = Command::new("sh")
            .arg("-c")
            .arg(cmd)
            .output()?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        let combined = if stderr.is_empty() {
            stdout.to_string()
        } else {
            format!("{stdout}\n[stderr]\n{stderr}")
        };

        Ok(ToolOutput {
            success: output.status.success(),
            output: combined,
        })
    }
}
