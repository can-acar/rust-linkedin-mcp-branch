use crate::tools::{call_tool, get_tools, ServerState};
use serde_json::{json, Value};
use std::sync::Arc;
use tokio::sync::Mutex;
 
pub async fn handle_request(state: Arc<Mutex<ServerState>>, msg: Value) -> Option<Value> {
    let method = msg.get("method")?.as_str()?;
    let id = msg.get("id").cloned();
 
    // "initialized" bildirimi - yanıt gerektirmez
    if id.is_none() {
        return None;
    }
    let id = id.unwrap();
 
    let result: Result<Value, String> = match method {
        "initialize" => Ok(json!({
            "protocolVersion": "2024-11-05",
            "capabilities": { "tools": {} },
            "serverInfo": { "name": "linkedin-mcp", "version": "0.1.0" }
        })),
 
        "tools/list" => Ok(json!({ "tools": get_tools() })),
 
        "tools/call" => {
            let params = msg.get("params").cloned().unwrap_or(json!({}));
            let name = params
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let args = params
                .get("arguments")
                .cloned()
                .unwrap_or(json!({}));
 
            match call_tool(Arc::clone(&state), &name, args).await {
                Ok(text) => Ok(json!({
                    "content": [{ "type": "text", "text": text }]
                })),
                Err(e) => Ok(json!({
                    "content": [{ "type": "text", "text": format!("Hata: {}", e) }],
                    "isError": true
                })),
            }
        }
 
        "ping" => Ok(json!({})),
 
        _ => Err(format!("Bilinmeyen metot: {}", method)),
    };
 
    Some(match result {
        Ok(r) => json!({ "jsonrpc": "2.0", "id": id, "result": r }),
        Err(e) => json!({
            "jsonrpc": "2.0",
            "id": id,
            "error": { "code": -32601, "message": e }
        }),
    })
}