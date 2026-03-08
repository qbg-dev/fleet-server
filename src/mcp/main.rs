//! boring-mail-mcp — MCP stdio wrapper for boring_mail_server
//!
//! A thin JSON-RPC 2.0 proxy that exposes the boring-mail HTTP API
//! as MCP tools over stdin/stdout.
//!
//! Configuration via environment variables:
//!   BORING_MAIL_URL   — base URL of the HTTP server (default: http://localhost:8025)
//!   BORING_MAIL_TOKEN — bearer token for authentication (required)

mod protocol;
mod tools;

use std::io::{self, BufRead, Write};

use protocol::{JsonRpcRequest, JsonRpcResponse, McpError};
use tools::ToolHandler;

fn main() {
    let url = std::env::var("BORING_MAIL_URL")
        .unwrap_or_else(|_| "http://localhost:8025".to_string());
    let token = std::env::var("BORING_MAIL_TOKEN")
        .unwrap_or_default();

    let handler = ToolHandler::new(&url, &token);
    let rt = tokio::runtime::Runtime::new().expect("failed to create tokio runtime");

    let stdin = io::stdin();
    let mut stdout = io::stdout();

    for line in stdin.lock().lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => break,
        };

        let line = line.trim().to_string();
        if line.is_empty() {
            continue;
        }

        let request: JsonRpcRequest = match serde_json::from_str(&line) {
            Ok(r) => r,
            Err(e) => {
                let resp = JsonRpcResponse::error(
                    serde_json::Value::Null,
                    McpError::parse_error(&format!("invalid JSON: {e}")),
                );
                write_response(&mut stdout, &resp);
                continue;
            }
        };

        let response = rt.block_on(handle_request(&handler, &request));

        if let Some(resp) = response {
            write_response(&mut stdout, &resp);
        }
    }
}

async fn handle_request(handler: &ToolHandler, req: &JsonRpcRequest) -> Option<JsonRpcResponse> {
    let id = req.id.clone().unwrap_or(serde_json::Value::Null);

    match req.method.as_str() {
        "initialize" => Some(JsonRpcResponse::success(id, protocol::initialize_result())),

        "notifications/initialized" => None, // notification, no response

        "tools/list" => Some(JsonRpcResponse::success(id, tools::list_tools())),

        "tools/call" => {
            let params = req.params.as_ref();
            let tool_name = params
                .and_then(|p| p.get("name"))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let arguments = params
                .and_then(|p| p.get("arguments"))
                .cloned()
                .unwrap_or(serde_json::Value::Object(serde_json::Map::new()));

            match handler.call(tool_name, &arguments).await {
                Ok(result) => Some(JsonRpcResponse::success(id, result)),
                Err(e) => Some(JsonRpcResponse::success(
                    id,
                    serde_json::json!({
                        "content": [{"type": "text", "text": format!("Error: {e}")}],
                        "isError": true,
                    }),
                )),
            }
        }

        "ping" => Some(JsonRpcResponse::success(id, serde_json::json!({}))),

        _ => {
            // Unknown method — if it has an id, respond with error
            if req.id.is_some() {
                Some(JsonRpcResponse::error(
                    id,
                    McpError::method_not_found(&req.method),
                ))
            } else {
                None // notification
            }
        }
    }
}

fn write_response(stdout: &mut io::Stdout, resp: &JsonRpcResponse) {
    let json = serde_json::to_string(resp).expect("failed to serialize response");
    let _ = writeln!(stdout, "{json}");
    let _ = stdout.flush();
}
