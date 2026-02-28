use super::{Tool, ToolDef, ToolOutput};
use anyhow::Result;
use serde_json::json;

pub struct ReadFile;
pub struct WriteFile;

impl Tool for ReadFile {
    fn definition(&self) -> ToolDef {
        ToolDef {
            name: "read_file".into(),
            description: "Read the contents of a file".into(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "File path to read" }
                },
                "required": ["path"]
            }),
        }
    }

    fn needs_approval(&self) -> bool { false }

    fn execute(&self, args: serde_json::Value) -> Result<ToolOutput> {
        let path = args["path"].as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing 'path' argument"))?;
        match std::fs::read_to_string(path) {
            Ok(content) => Ok(ToolOutput { success: true, output: content }),
            Err(e) => Ok(ToolOutput { success: false, output: format!("Error: {e}") }),
        }
    }
}

impl Tool for WriteFile {
    fn definition(&self) -> ToolDef {
        ToolDef {
            name: "write_file".into(),
            description: "Write content to a file".into(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "File path to write" },
                    "content": { "type": "string", "description": "Content to write" }
                },
                "required": ["path", "content"]
            }),
        }
    }

    fn execute(&self, args: serde_json::Value) -> Result<ToolOutput> {
        let path = args["path"].as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing 'path' argument"))?;
        let content = args["content"].as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing 'content' argument"))?;

        if let Some(parent) = std::path::Path::new(path).parent() {
            std::fs::create_dir_all(parent)?;
        }
        match std::fs::write(path, content) {
            Ok(()) => Ok(ToolOutput { success: true, output: format!("Written to {path}") }),
            Err(e) => Ok(ToolOutput { success: false, output: format!("Error: {e}") }),
        }
    }
}
