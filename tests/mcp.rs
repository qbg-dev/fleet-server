/// Integration tests for the boring-mail-mcp binary.
/// These test the JSON-RPC protocol and tool routing without a running HTTP server
/// (tools/call will fail with connection refused, but protocol handling is verified).
use std::io::Write;
use std::process::{Command, Stdio};

fn run_mcp(input: &str) -> Vec<serde_json::Value> {
    let mut child = Command::new("target/debug/boring-mail-mcp")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .env("BORING_MAIL_TOKEN", "test-token")
        .env("BORING_MAIL_URL", "http://127.0.0.1:1") // unreachable
        .spawn()
        .expect("failed to spawn boring-mail-mcp");

    child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(input.as_bytes())
        .unwrap();
    drop(child.stdin.take());

    let output = child.wait_with_output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    stdout
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| serde_json::from_str(l).expect("invalid JSON in output"))
        .collect()
}

#[test]
fn test_mcp_initialize() {
    let responses = run_mcp(
        r#"{"jsonrpc":"2.0","method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test","version":"1.0"}},"id":1}"#,
    );
    assert_eq!(responses.len(), 1);
    let r = &responses[0];
    assert_eq!(r["jsonrpc"], "2.0");
    assert_eq!(r["id"], 1);
    assert_eq!(r["result"]["protocolVersion"], "2024-11-05");
    assert_eq!(r["result"]["serverInfo"]["name"], "boring-mail-mcp");
    assert!(r["result"]["capabilities"]["tools"].is_object());
}

#[test]
fn test_mcp_tools_list() {
    let input = concat!(
        r#"{"jsonrpc":"2.0","method":"initialize","params":{},"id":1}"#, "\n",
        r#"{"jsonrpc":"2.0","method":"tools/list","params":{},"id":2}"#, "\n",
    );
    let responses = run_mcp(input);
    assert_eq!(responses.len(), 2);

    let tools = &responses[1]["result"]["tools"];
    assert!(tools.is_array());
    let tool_names: Vec<&str> = tools
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["name"].as_str().unwrap())
        .collect();

    assert!(tool_names.contains(&"send_message"));
    assert!(tool_names.contains(&"read_inbox"));
    assert!(tool_names.contains(&"get_message"));
    assert!(tool_names.contains(&"search_messages"));
    assert!(tool_names.contains(&"modify_labels"));
    assert!(tool_names.contains(&"trash_message"));
    assert!(tool_names.contains(&"list_labels"));
    assert!(tool_names.contains(&"list_threads"));
    assert!(tool_names.contains(&"get_thread"));
    assert_eq!(tool_names.len(), 9);
}

#[test]
fn test_mcp_ping() {
    let responses = run_mcp(r#"{"jsonrpc":"2.0","method":"ping","id":3}"#);
    assert_eq!(responses.len(), 1);
    assert_eq!(responses[0]["id"], 3);
    assert!(responses[0]["result"].is_object());
}

#[test]
fn test_mcp_unknown_tool() {
    let input = r#"{"jsonrpc":"2.0","method":"tools/call","params":{"name":"nonexistent","arguments":{}},"id":4}"#;
    let responses = run_mcp(input);
    assert_eq!(responses.len(), 1);
    assert_eq!(responses[0]["result"]["isError"], true);
    assert!(responses[0]["result"]["content"][0]["text"]
        .as_str()
        .unwrap()
        .contains("unknown tool"));
}

#[test]
fn test_mcp_tool_call_missing_args() {
    let input = r#"{"jsonrpc":"2.0","method":"tools/call","params":{"name":"send_message","arguments":{}},"id":5}"#;
    let responses = run_mcp(input);
    assert_eq!(responses.len(), 1);
    assert_eq!(responses[0]["result"]["isError"], true);
    assert!(responses[0]["result"]["content"][0]["text"]
        .as_str()
        .unwrap()
        .contains("missing required argument"));
}

#[test]
fn test_mcp_notification_no_response() {
    // notifications/initialized should produce no response
    let input = concat!(
        r#"{"jsonrpc":"2.0","method":"initialize","params":{},"id":1}"#, "\n",
        r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#, "\n",
        r#"{"jsonrpc":"2.0","method":"ping","id":2}"#, "\n",
    );
    let responses = run_mcp(input);
    // Should get 2 responses (initialize + ping), not 3
    assert_eq!(responses.len(), 2);
    assert_eq!(responses[0]["id"], 1);
    assert_eq!(responses[1]["id"], 2);
}

#[test]
fn test_mcp_parse_error() {
    let responses = run_mcp("this is not json\n");
    assert_eq!(responses.len(), 1);
    assert!(responses[0]["error"].is_object());
    assert_eq!(responses[0]["error"]["code"], -32700);
}

#[test]
fn test_mcp_method_not_found() {
    let responses = run_mcp(r#"{"jsonrpc":"2.0","method":"bogus/method","id":6}"#);
    assert_eq!(responses.len(), 1);
    assert!(responses[0]["error"].is_object());
    assert_eq!(responses[0]["error"]["code"], -32601);
}
