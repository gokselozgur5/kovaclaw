use anyhow::Result;
use kova_core::agent::Agent;
use kova_core::config::Config;
use kova_core::llm::LlmClient;
use kova_core::session::Session;
use std::io::{self, BufRead, Write};
use std::path::PathBuf;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let base_dir = find_project_root()?;
    let config_path = base_dir.join("config/kovaclaw.json");
    let config = Config::load(&config_path)?;
    let identity = config.load_identity(&base_dir)?;

    let session_dir = base_dir.join(&config.session_dir);
    let session_id = chrono::Utc::now().format("%Y%m%d_%H%M%S").to_string();
    let session = Session::new(&session_dir, &session_id)?;

    let llm = LlmClient::new(config.llm);
    let mut agent = Agent::new(llm, identity).with_session(session)?;

    println!("KovaClaw v0.2.0 (session: {session_id})");
    println!("Tools: read_file, write_file, shell_exec");
    println!("Ctrl+C or 'exit' to quit\n");

    let stdin = io::stdin();
    let mut stdout = io::stdout();

    loop {
        print!("kova> ");
        stdout.flush()?;

        let mut input = String::new();
        if stdin.lock().read_line(&mut input)? == 0 {
            break;
        }

        let input = input.trim();
        if input.is_empty() { continue; }
        if input == "exit" || input == "quit" { break; }

        // Stream response
        print!("\n");
        let mut writer = tokio::io::stdout();
        let response = match agent.send_stream(input, &mut writer).await {
            Ok(r) => r,
            Err(e) => {
                // Fallback to non-streaming
                eprintln!("[stream failed, trying non-stream: {e}]");
                match agent.send(input).await {
                    Ok(r) => { print!("{r}"); r },
                    Err(e2) => { eprintln!("[error] {e2}\n"); continue; }
                }
            }
        };
        println!("\n");

        // Check for tool calls in response
        let tool_calls = Agent::parse_tool_calls(&response);
        for call in tool_calls {
            let tool = match agent.tools.get(&call.name) {
                Some(t) => t,
                None => {
                    eprintln!("[unknown tool: {}]", call.name);
                    continue;
                }
            };

            // Approval flow
            if tool.needs_approval() {
                print!("[tool: {} | args: {}] approve? (y/n) ", call.name, call.arguments);
                stdout.flush()?;
                let mut answer = String::new();
                stdin.lock().read_line(&mut answer)?;
                if answer.trim() != "y" {
                    agent.feed_tool_result(&call.name, "Tool call denied by user.");
                    println!("[denied]\n");
                    continue;
                }
            } else {
                println!("[tool: {} | auto-approved]", call.name);
            }

            // Execute
            match agent.tools.execute(&call) {
                Ok(result) => {
                    let preview = if result.output.len() > 200 {
                        format!("{}...", &result.output[..200])
                    } else {
                        result.output.clone()
                    };
                    println!("[result: {}]\n{}\n", if result.success { "ok" } else { "fail" }, preview);
                    agent.feed_tool_result(&call.name, &result.output);
                }
                Err(e) => {
                    let err = format!("Execution error: {e}");
                    eprintln!("[{err}]\n");
                    agent.feed_tool_result(&call.name, &err);
                }
            }

            // Get follow-up response after tool result
            print!("kova: ");
            stdout.flush()?;
            let mut writer = tokio::io::stdout();
            match agent.send_stream("", &mut writer).await {
                Ok(_) => {},
                Err(_) => {
                    if let Ok(r) = agent.send("").await {
                        print!("{r}");
                    }
                }
            }
            println!("\n");
        }
    }

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
