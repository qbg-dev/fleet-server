use reqwest::Client;
use serde_json::{json, Value};
use std::net::TcpListener;
async fn spawn_server() -> (String, Client) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);

    let tmp = tempfile::tempdir().unwrap();
    let data_dir = tmp.path().to_path_buf();
    let config = boring_mail::config::Config {
        bind_addr: format!("127.0.0.1:{port}"),
        db_path: data_dir.join("mail.db"),
        blob_dir: data_dir.join("blobs"),
        data_dir,
        admin_token: None,
    };

    let db = boring_mail::db::connection::setup(&config).await.unwrap();
    let app = boring_mail::api::router(db, &config);

    let listener = tokio::net::TcpListener::bind(format!("127.0.0.1:{port}"))
        .await
        .unwrap();

    // Keep tempdir alive
    tokio::spawn(async move {
        let _tmp = tmp;
        axum::serve(listener, app).await.unwrap();
    });

    let base = format!("http://127.0.0.1:{port}");
    let client = Client::new();
    (base, client)
}

#[tokio::test]
async fn test_health() {
    let (base, client) = spawn_server().await;
    let resp = client.get(format!("{base}/health")).send().await.unwrap();
    assert_eq!(resp.status(), 200);
    let body: Value = resp.json().await.unwrap();
    assert_eq!(body["status"], "ok");
}

#[tokio::test]
async fn test_register_account() {
    let (base, client) = spawn_server().await;
    let resp = client
        .post(format!("{base}/api/accounts"))
        .json(&json!({"name": "agent-1", "display_name": "Agent One"}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body: Value = resp.json().await.unwrap();
    assert_eq!(body["name"], "agent-1");
    assert!(body["bearerToken"].is_string());
    assert!(!body["bearerToken"].as_str().unwrap().is_empty());
}

#[tokio::test]
async fn test_full_message_flow() {
    let (base, client) = spawn_server().await;

    // Register sender and recipient
    let sender: Value = client
        .post(format!("{base}/api/accounts"))
        .json(&json!({"name": "sender"}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let recipient: Value = client
        .post(format!("{base}/api/accounts"))
        .json(&json!({"name": "recipient"}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();

    let sender_token = sender["bearerToken"].as_str().unwrap();
    let recipient_token = recipient["bearerToken"].as_str().unwrap();
    let recipient_id = recipient["id"].as_str().unwrap();

    // Send message
    let sent: Value = client
        .post(format!("{base}/api/messages/send"))
        .bearer_auth(sender_token)
        .json(&json!({
            "to": [recipient_id],
            "subject": "Hello from integration test",
            "body": "This is the body"
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();

    let msg_id = sent["id"].as_str().unwrap();
    assert!(!msg_id.is_empty());

    // Recipient lists inbox
    let inbox: Value = client
        .get(format!("{base}/api/messages?label=INBOX"))
        .bearer_auth(recipient_token)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(inbox["messages"].as_array().unwrap().len(), 1);
    assert_eq!(inbox["messages"][0]["subject"], "Hello from integration test");

    // Recipient gets message (auto-removes UNREAD)
    let msg: Value = client
        .get(format!("{base}/api/messages/{msg_id}"))
        .bearer_auth(recipient_token)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(msg["subject"], "Hello from integration test");
    assert_eq!(msg["body"], "This is the body");

    // Sender lists SENT
    let sent_list: Value = client
        .get(format!("{base}/api/messages?label=SENT"))
        .bearer_auth(sender_token)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(sent_list["messages"].as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn test_modify_labels() {
    let (base, client) = spawn_server().await;

    let sender: Value = client
        .post(format!("{base}/api/accounts"))
        .json(&json!({"name": "sender"}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let recipient: Value = client
        .post(format!("{base}/api/accounts"))
        .json(&json!({"name": "recipient"}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();

    let sender_token = sender["bearerToken"].as_str().unwrap();
    let recipient_token = recipient["bearerToken"].as_str().unwrap();

    let sent: Value = client
        .post(format!("{base}/api/messages/send"))
        .bearer_auth(sender_token)
        .json(&json!({
            "to": [recipient["id"].as_str().unwrap()],
            "subject": "Modify test",
            "body": "Body"
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();

    let msg_id = sent["id"].as_str().unwrap();

    let modified: Value = client
        .post(format!("{base}/api/messages/{msg_id}/modify"))
        .bearer_auth(recipient_token)
        .json(&json!({
            "addLabelIds": ["STARRED"],
            "removeLabelIds": ["UNREAD"]
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();

    let labels = modified["labelIds"].as_array().unwrap();
    assert!(labels.iter().any(|l| l == "STARRED"));
    assert!(!labels.iter().any(|l| l == "UNREAD"));
    assert!(labels.iter().any(|l| l == "INBOX"));
}

#[tokio::test]
async fn test_trash_and_delete() {
    let (base, client) = spawn_server().await;

    let sender: Value = client
        .post(format!("{base}/api/accounts"))
        .json(&json!({"name": "sender"}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let recipient: Value = client
        .post(format!("{base}/api/accounts"))
        .json(&json!({"name": "recipient"}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();

    let sender_token = sender["bearerToken"].as_str().unwrap();
    let recipient_token = recipient["bearerToken"].as_str().unwrap();

    let sent: Value = client
        .post(format!("{base}/api/messages/send"))
        .bearer_auth(sender_token)
        .json(&json!({
            "to": [recipient["id"].as_str().unwrap()],
            "subject": "Trash me",
            "body": "Body"
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();

    let msg_id = sent["id"].as_str().unwrap();

    // Trash
    let resp = client
        .post(format!("{base}/api/messages/{msg_id}/trash"))
        .bearer_auth(recipient_token)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    // Inbox should be empty
    let inbox: Value = client
        .get(format!("{base}/api/messages?label=INBOX"))
        .bearer_auth(recipient_token)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(inbox["messages"].as_array().unwrap().len(), 0);

    // Permanently delete
    let resp = client
        .delete(format!("{base}/api/messages/{msg_id}"))
        .bearer_auth(sender_token)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
}

#[tokio::test]
async fn test_unauthorized_access() {
    let (base, client) = spawn_server().await;

    let resp = client
        .get(format!("{base}/api/messages"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 401);

    let resp = client
        .get(format!("{base}/api/messages"))
        .bearer_auth("invalid-token")
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 401);
}

#[tokio::test]
async fn test_labels_endpoint() {
    let (base, client) = spawn_server().await;

    let sender: Value = client
        .post(format!("{base}/api/accounts"))
        .json(&json!({"name": "sender"}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let recipient: Value = client
        .post(format!("{base}/api/accounts"))
        .json(&json!({"name": "recipient"}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();

    let sender_token = sender["bearerToken"].as_str().unwrap();
    let recipient_token = recipient["bearerToken"].as_str().unwrap();

    client
        .post(format!("{base}/api/messages/send"))
        .bearer_auth(sender_token)
        .json(&json!({
            "to": [recipient["id"].as_str().unwrap()],
            "subject": "Test",
            "body": "Body"
        }))
        .send()
        .await
        .unwrap();

    let labels: Value = client
        .get(format!("{base}/api/labels"))
        .bearer_auth(recipient_token)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();

    let label_list = labels["labels"].as_array().unwrap();
    let inbox_label = label_list.iter().find(|l| l["name"] == "INBOX").unwrap();
    assert_eq!(inbox_label["messagesTotal"], 1);
    assert_eq!(inbox_label["messagesUnread"], 1);
}

#[tokio::test]
async fn test_threads_endpoint() {
    let (base, client) = spawn_server().await;

    let sender: Value = client
        .post(format!("{base}/api/accounts"))
        .json(&json!({"name": "sender"}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let recipient: Value = client
        .post(format!("{base}/api/accounts"))
        .json(&json!({"name": "recipient"}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();

    let sender_token = sender["bearerToken"].as_str().unwrap();
    let recipient_token = recipient["bearerToken"].as_str().unwrap();
    let recipient_id = recipient["id"].as_str().unwrap();
    let sender_id = sender["id"].as_str().unwrap();

    let sent: Value = client
        .post(format!("{base}/api/messages/send"))
        .bearer_auth(sender_token)
        .json(&json!({
            "to": [recipient_id],
            "subject": "Thread test",
            "body": "First"
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();

    let msg_id = sent["id"].as_str().unwrap();

    client
        .post(format!("{base}/api/messages/send"))
        .bearer_auth(recipient_token)
        .json(&json!({
            "to": [sender_id],
            "subject": "Re: Thread test",
            "body": "Reply",
            "in_reply_to": msg_id,
        }))
        .send()
        .await
        .unwrap();

    // List threads
    let threads: Value = client
        .get(format!("{base}/api/threads?label=INBOX"))
        .bearer_auth(recipient_token)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(threads["threads"].as_array().unwrap().len() >= 1);

    // Get thread
    let thread_id = sent["threadId"].as_str().unwrap();
    let thread: Value = client
        .get(format!("{base}/api/threads/{thread_id}"))
        .bearer_auth(recipient_token)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(thread["messageCount"], 2);
    assert_eq!(thread["messages"].as_array().unwrap().len(), 2);
}

#[tokio::test]
async fn test_diagnostics_in_response() {
    let (base, client) = spawn_server().await;

    let sender: Value = client
        .post(format!("{base}/api/accounts"))
        .json(&json!({"name": "sender"}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let recipient: Value = client
        .post(format!("{base}/api/accounts"))
        .json(&json!({"name": "recipient"}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();

    let sender_token = sender["bearerToken"].as_str().unwrap();
    let recipient_token = recipient["bearerToken"].as_str().unwrap();

    // Send a message with reply_by
    client
        .post(format!("{base}/api/messages/send"))
        .bearer_auth(sender_token)
        .json(&json!({
            "to": [recipient["id"].as_str().unwrap()],
            "subject": "Urgent",
            "body": "Please respond",
            "reply_by": "2026-03-09T00:00:00Z"
        }))
        .send()
        .await
        .unwrap();

    // Any authenticated request should include _diagnostics
    let inbox: Value = client
        .get(format!("{base}/api/messages?label=INBOX"))
        .bearer_auth(recipient_token)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();

    let diag = &inbox["_diagnostics"];
    assert!(diag.is_object(), "_diagnostics missing from response");
    assert_eq!(diag["unread_count"], 1);
    assert_eq!(diag["pending_replies"].as_array().unwrap().len(), 1);
    assert_eq!(diag["pending_replies"][0]["subject"], "Urgent");
    assert!(diag["inbox_hint"].is_string());
}

#[tokio::test]
async fn test_search_endpoint() {
    let (base, client) = spawn_server().await;

    let sender: Value = client
        .post(format!("{base}/api/accounts"))
        .json(&json!({"name": "sender"}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let recipient: Value = client
        .post(format!("{base}/api/accounts"))
        .json(&json!({"name": "recipient"}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();

    let sender_token = sender["bearerToken"].as_str().unwrap();
    let recipient_token = recipient["bearerToken"].as_str().unwrap();
    let recipient_id = recipient["id"].as_str().unwrap();

    // Send two different messages
    client
        .post(format!("{base}/api/messages/send"))
        .bearer_auth(sender_token)
        .json(&json!({
            "to": [recipient_id],
            "subject": "Deployment success",
            "body": "Production deploy completed"
        }))
        .send()
        .await
        .unwrap();

    client
        .post(format!("{base}/api/messages/send"))
        .bearer_auth(sender_token)
        .json(&json!({
            "to": [recipient_id],
            "subject": "Code review",
            "body": "Please review PR #42"
        }))
        .send()
        .await
        .unwrap();

    // Search for "deploy"
    let results: Value = client
        .get(format!("{base}/api/search?q=deploy"))
        .bearer_auth(recipient_token)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(results["resultSizeEstimate"], 1);
    assert_eq!(results["messages"][0]["subject"], "Deployment success");

    // Search for "review"
    let results: Value = client
        .get(format!("{base}/api/search?q=review"))
        .bearer_auth(recipient_token)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(results["resultSizeEstimate"], 1);
    assert_eq!(results["messages"][0]["subject"], "Code review");
}

#[tokio::test]
async fn test_batch_modify() {
    let (base, client) = spawn_server().await;

    let sender: Value = client
        .post(format!("{base}/api/accounts"))
        .json(&json!({"name": "sender"}))
        .send().await.unwrap().json().await.unwrap();
    let recipient: Value = client
        .post(format!("{base}/api/accounts"))
        .json(&json!({"name": "recipient"}))
        .send().await.unwrap().json().await.unwrap();

    let sender_token = sender["bearerToken"].as_str().unwrap();
    let recipient_token = recipient["bearerToken"].as_str().unwrap();
    let recipient_id = recipient["id"].as_str().unwrap();

    // Send 3 messages
    let mut msg_ids = Vec::new();
    for i in 0..3 {
        let sent: Value = client
            .post(format!("{base}/api/messages/send"))
            .bearer_auth(sender_token)
            .json(&json!({
                "to": [recipient_id],
                "subject": format!("Batch {i}"),
                "body": "Body"
            }))
            .send().await.unwrap().json().await.unwrap();
        msg_ids.push(sent["id"].as_str().unwrap().to_string());
    }

    // Batch mark as read
    let resp: Value = client
        .post(format!("{base}/api/messages/batchModify"))
        .bearer_auth(recipient_token)
        .json(&json!({
            "ids": msg_ids,
            "removeLabelIds": ["UNREAD"],
            "addLabelIds": ["STARRED"]
        }))
        .send().await.unwrap().json().await.unwrap();
    assert_eq!(resp["modified"], 3);

    // Verify labels changed
    let labels: Value = client
        .get(format!("{base}/api/labels"))
        .bearer_auth(recipient_token)
        .send().await.unwrap().json().await.unwrap();
    let label_list = labels["labels"].as_array().unwrap();
    let starred = label_list.iter().find(|l| l["name"] == "STARRED").unwrap();
    assert_eq!(starred["messagesTotal"], 3);
}

#[tokio::test]
async fn test_custom_labels() {
    let (base, client) = spawn_server().await;

    let account: Value = client
        .post(format!("{base}/api/accounts"))
        .json(&json!({"name": "agent-1"}))
        .send().await.unwrap().json().await.unwrap();
    let token = account["bearerToken"].as_str().unwrap();

    // Create custom label
    let label: Value = client
        .post(format!("{base}/api/labels"))
        .bearer_auth(token)
        .json(&json!({"name": "MY_CUSTOM"}))
        .send().await.unwrap().json().await.unwrap();
    assert_eq!(label["name"], "MY_CUSTOM");
    assert_eq!(label["type"], "user");

    // Delete custom label
    let resp = client
        .delete(format!("{base}/api/labels/MY_CUSTOM"))
        .bearer_auth(token)
        .send().await.unwrap();
    assert_eq!(resp.status(), 200);
}

#[tokio::test]
async fn test_mailing_lists() {
    let (base, client) = spawn_server().await;

    let alice: Value = client
        .post(format!("{base}/api/accounts"))
        .json(&json!({"name": "alice"}))
        .send().await.unwrap().json().await.unwrap();
    let bob: Value = client
        .post(format!("{base}/api/accounts"))
        .json(&json!({"name": "bob"}))
        .send().await.unwrap().json().await.unwrap();

    let alice_token = alice["bearerToken"].as_str().unwrap();
    let bob_token = bob["bearerToken"].as_str().unwrap();

    // Create mailing list
    let list: Value = client
        .post(format!("{base}/api/lists"))
        .bearer_auth(alice_token)
        .json(&json!({"name": "team-all", "description": "All team members"}))
        .send().await.unwrap().json().await.unwrap();
    let list_id = list["id"].as_str().unwrap();
    assert_eq!(list["name"], "team-all");

    // Subscribe both
    client
        .post(format!("{base}/api/lists/{list_id}/subscribe"))
        .bearer_auth(alice_token)
        .send().await.unwrap();
    client
        .post(format!("{base}/api/lists/{list_id}/subscribe"))
        .bearer_auth(bob_token)
        .send().await.unwrap();

    // Unsubscribe and resubscribe works
    client
        .post(format!("{base}/api/lists/{list_id}/unsubscribe"))
        .bearer_auth(bob_token)
        .send().await.unwrap();
    client
        .post(format!("{base}/api/lists/{list_id}/subscribe"))
        .bearer_auth(bob_token)
        .send().await.unwrap();
}

#[tokio::test]
async fn test_webhook_git_commit() {
    let (base, client) = spawn_server().await;

    let author: Value = client
        .post(format!("{base}/api/accounts"))
        .json(&json!({"name": "developer"}))
        .send().await.unwrap().json().await.unwrap();
    let recipient: Value = client
        .post(format!("{base}/api/accounts"))
        .json(&json!({"name": "reviewer"}))
        .send().await.unwrap().json().await.unwrap();

    let recipient_id = recipient["id"].as_str().unwrap();
    let recipient_token = recipient["bearerToken"].as_str().unwrap();

    // Send commit webhook
    let resp: Value = client
        .post(format!("{base}/api/webhooks/git-commit"))
        .json(&json!({
            "author": "developer",
            "sha": "abc1234def5678",
            "message": "fix: resolve login bug",
            "recipients": [recipient_id]
        }))
        .send().await.unwrap().json().await.unwrap();
    assert_eq!(resp["delivered"], 1);

    // Recipient should see the commit message in inbox
    let inbox: Value = client
        .get(format!("{base}/api/messages?label=INBOX"))
        .bearer_auth(recipient_token)
        .send().await.unwrap().json().await.unwrap();
    let msgs = inbox["messages"].as_array().unwrap();
    assert_eq!(msgs.len(), 1);
    assert!(msgs[0]["subject"].as_str().unwrap().contains("fix: resolve login bug"));
}
