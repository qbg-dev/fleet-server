/// Performance benchmarks for boring-mail hot paths.
/// Run with: cargo test --test bench -- --nocapture
use reqwest::Client;
use serde_json::{json, Value};
use std::net::TcpListener;
use std::time::Instant;

async fn spawn_server() -> (String, Client) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);

    let tmp = tempfile::tempdir().unwrap();
    let data_dir = tmp.path().to_path_buf();
    let base_url = std::env::var("BORING_MAIL_TEST_DB_BASE")
        .unwrap_or_else(|_| "mysql://root@localhost:3307".to_string());
    let db_name = format!("test_bench_{}", uuid::Uuid::new_v4().simple());
    let config = boring_mail::config::Config {
        bind_addr: format!("127.0.0.1:{port}"),
        database_url: format!("{base_url}/{db_name}"),
        max_db_connections: 5,
        blob_dir: data_dir.join("blobs"),
        data_dir,
        admin_token: None,
        registry_path: None,
        max_body_size: 10 * 1024 * 1024,
        request_timeout_secs: 30,
        rate_limit_per_minute: 0, // unlimited for benchmarks
    };

    let db = boring_mail::db::connection::setup(&config).await.unwrap();
    let app = boring_mail::api::router(db, &config);

    let listener = tokio::net::TcpListener::bind(format!("127.0.0.1:{port}"))
        .await
        .unwrap();

    tokio::spawn(async move {
        let _tmp = tmp;
        axum::serve(listener, app).await.unwrap();
    });

    (format!("http://127.0.0.1:{port}"), Client::new())
}

async fn register(client: &Client, base: &str, name: &str) -> (String, String) {
    let resp = client
        .post(format!("{base}/api/accounts"))
        .json(&json!({"name": name}))
        .send()
        .await
        .unwrap();
    let body: Value = resp.json().await.unwrap();
    let id = body["id"].as_str().unwrap().to_string();
    let token = body["bearerToken"].as_str().unwrap().to_string();
    (id, token)
}

async fn send_msg(client: &Client, base: &str, token: &str, to: &str, subject: &str, body: &str) -> String {
    let resp = client
        .post(format!("{base}/api/messages/send"))
        .bearer_auth(token)
        .json(&json!({"to": [to], "subject": subject, "body": body}))
        .send()
        .await
        .unwrap();
    let val: Value = resp.json().await.unwrap();
    val["id"].as_str().unwrap().to_string()
}

#[tokio::test]
async fn bench_message_operations() {
    let (base, client) = spawn_server().await;

    let (alice_id, alice_token) = register(&client, &base, "bench-alice").await;
    let (bob_id, bob_token) = register(&client, &base, "bench-bob").await;
    let _ = &alice_id; // used by sender auth

    // Warm up
    for _ in 0..5 {
        send_msg(&client, &base, &alice_token, &bob_id, "warmup", "body").await;
    }

    // Benchmark: send_message
    let n: u32 = 100;
    let start = Instant::now();
    for i in 0..n {
        send_msg(
            &client,
            &base,
            &alice_token,
            &bob_id,
            &format!("bench msg {i}"),
            "benchmark body content for performance testing",
        )
        .await;
    }
    let send_elapsed = start.elapsed();
    let send_per_op = send_elapsed / n;

    // Benchmark: list_messages
    let list_n: u32 = 100;
    let start = Instant::now();
    for _ in 0..list_n {
        let resp = client
            .get(format!("{base}/api/messages?label=INBOX&maxResults=20"))
            .bearer_auth(&bob_token)
            .send()
            .await
            .unwrap();
        assert!(resp.status().is_success());
    }
    let list_elapsed = start.elapsed();
    let list_per_op = list_elapsed / list_n;

    // Benchmark: get_message
    let resp = client
        .get(format!("{base}/api/messages?label=INBOX&maxResults=1"))
        .bearer_auth(&bob_token)
        .send()
        .await
        .unwrap();
    let body: Value = resp.json().await.unwrap();
    let msg_id = body["messages"][0]["id"].as_str().unwrap().to_string();

    let get_n: u32 = 200;
    let start = Instant::now();
    for _ in 0..get_n {
        let resp = client
            .get(format!("{base}/api/messages/{msg_id}"))
            .bearer_auth(&bob_token)
            .send()
            .await
            .unwrap();
        assert!(resp.status().is_success());
    }
    let get_elapsed = start.elapsed();
    let get_per_op = get_elapsed / get_n;

    // Benchmark: search
    let search_n: u32 = 50;
    let start = Instant::now();
    for _ in 0..search_n {
        let resp = client
            .get(format!("{base}/api/search?q=benchmark&maxResults=10"))
            .bearer_auth(&bob_token)
            .send()
            .await
            .unwrap();
        assert!(resp.status().is_success());
    }
    let search_elapsed = start.elapsed();
    let search_per_op = search_elapsed / search_n;

    // Benchmark: list_labels
    let label_n: u32 = 200;
    let start = Instant::now();
    for _ in 0..label_n {
        let resp = client
            .get(format!("{base}/api/labels"))
            .bearer_auth(&bob_token)
            .send()
            .await
            .unwrap();
        assert!(resp.status().is_success());
    }
    let label_elapsed = start.elapsed();
    let label_per_op = label_elapsed / label_n;

    println!("\n=== boring-mail Performance Benchmarks ===");
    println!("send_message:  {send_per_op:>8.2?}/op  ({n} ops in {send_elapsed:.2?})");
    println!("list_messages: {list_per_op:>8.2?}/op  ({list_n} ops in {list_elapsed:.2?})");
    println!("get_message:   {get_per_op:>8.2?}/op  ({get_n} ops in {get_elapsed:.2?})");
    println!("search:        {search_per_op:>8.2?}/op  ({search_n} ops in {search_elapsed:.2?})");
    println!("list_labels:   {label_per_op:>8.2?}/op  ({label_n} ops in {label_elapsed:.2?})");
    println!("==========================================\n");

    // Assert performance targets (Dolt/MySQL has higher latency than SQLite)
    assert!(send_per_op.as_millis() < 50, "send_message too slow: {send_per_op:?}");
    assert!(list_per_op.as_millis() < 50, "list_messages too slow: {list_per_op:?}");
    assert!(get_per_op.as_millis() < 50, "get_message too slow: {get_per_op:?}");
    assert!(search_per_op.as_millis() < 100, "search too slow: {search_per_op:?}");
    assert!(label_per_op.as_millis() < 50, "list_labels too slow: {label_per_op:?}");
}
