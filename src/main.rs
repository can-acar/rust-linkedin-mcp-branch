mod credentials;
mod linkedin;
mod mcp;
mod tools;
 
use anyhow::Result;
use serde_json::Value;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::sync::Mutex;
 
#[tokio::main]
async fn main() -> Result<()> {
    let state = Arc::new(Mutex::new(tools::ServerState::new().await));
 
    let stdin = tokio::io::stdin();
    let mut stdout = tokio::io::stdout();
    let mut reader = BufReader::new(stdin);
    let mut line = String::new();
 
    loop {
        line.clear();
        let n = reader.read_line(&mut line).await?;
        if n == 0 {
            break;
        }
 
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
 
        let msg: Value = match serde_json::from_str(trimmed) {
            Ok(v) => v,
            Err(e) => {
                eprintln!("JSON parse hatası: {}", e);
                continue;
            }
        };
 
        if let Some(response) = mcp::handle_request(Arc::clone(&state), msg).await {
            let mut out = serde_json::to_string(&response)?;
            out.push('\n');
            stdout.write_all(out.as_bytes()).await?;
            stdout.flush().await?;
        }
    }
 
    Ok(())
}