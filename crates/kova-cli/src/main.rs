use anyhow::Result;
use kova_core::agent::Agent;
use kova_core::config::Config;
use kova_core::llm::LlmClient;
use std::io::{self, BufRead, Write};
use std::path::PathBuf;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let base_dir = find_project_root()?;
    let config_path = base_dir.join("config/kovaclaw.json");
    let config = Config::load(&config_path)?;
    let identity = config.load_identity(&base_dir)?;

    let llm = LlmClient::new(config.llm);
    let mut agent = Agent::new(llm, identity);

    println!("KovaClaw v0.1.0");
    println!("Connecting to LLM... (Ctrl+C or 'exit' to quit)");
    println!();

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
        if input.is_empty() {
            continue;
        }
        if input == "exit" || input == "quit" {
            break;
        }

        match agent.send(input).await {
            Ok(response) => {
                println!("\n{}\n", response);
            }
            Err(e) => {
                eprintln!("\n[hata] {}\n", e);
            }
        }
    }

    Ok(())
}

fn find_project_root() -> Result<PathBuf> {
    // Check env, then walk up from cwd looking for config/kovaclaw.json
    if let Ok(root) = std::env::var("KOVACLAW_ROOT") {
        return Ok(PathBuf::from(root));
    }

    let mut dir = std::env::current_dir()?;
    loop {
        if dir.join("config/kovaclaw.json").exists() {
            return Ok(dir);
        }
        if !dir.pop() {
            break;
        }
    }

    anyhow::bail!("kovaclaw.json not found. Set KOVACLAW_ROOT or run from the kovaclaw/ directory.")
}
