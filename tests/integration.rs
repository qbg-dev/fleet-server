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
        registry_path: None,
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

#[tokio::test]
async fn test_blob_upload_download() {
    let (base, client) = spawn_server().await;

    let account: Value = client
        .post(format!("{base}/api/accounts"))
        .json(&json!({"name": "agent-1"}))
        .send().await.unwrap().json().await.unwrap();
    let token = account["bearerToken"].as_str().unwrap();

    // Upload
    let resp: Value = client
        .post(format!("{base}/api/blobs"))
        .bearer_auth(token)
        .body("hello blob content")
        .send().await.unwrap().json().await.unwrap();

    let hash = resp["hash"].as_str().unwrap();
    assert!(!hash.is_empty());
    assert_eq!(resp["size"], 18);

    // Download
    let data = client
        .get(format!("{base}/api/blobs/{hash}"))
        .bearer_auth(token)
        .send().await.unwrap()
        .bytes().await.unwrap();
    assert_eq!(data.as_ref(), b"hello blob content");
}

#[tokio::test]
async fn test_send_message_with_attachments() {
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

    // Upload a blob first
    let blob: Value = client
        .post(format!("{base}/api/blobs"))
        .bearer_auth(sender_token)
        .body("attachment content here")
        .send().await.unwrap().json().await.unwrap();
    let blob_hash = blob["hash"].as_str().unwrap();

    // Send message with attachment
    let sent: Value = client
        .post(format!("{base}/api/messages/send"))
        .bearer_auth(sender_token)
        .json(&json!({
            "to": [recipient_id],
            "subject": "With attachment",
            "body": "See attached",
            "attachments": [blob_hash]
        }))
        .send().await.unwrap().json().await.unwrap();

    let msg_id = sent["id"].as_str().unwrap();

    // Get message should show attachments
    let msg: Value = client
        .get(format!("{base}/api/messages/{msg_id}"))
        .bearer_auth(recipient_token)
        .send().await.unwrap().json().await.unwrap();

    assert_eq!(msg["hasAttachments"], true);
    let attachments = msg["attachments"].as_array().unwrap();
    assert_eq!(attachments.len(), 1);
    assert_eq!(attachments[0]["blobHash"], blob_hash);
}

/// Conformance test: verify API response shapes match Gmail-like conventions
#[tokio::test]
async fn test_conformance_response_shapes() {
    let (base, client) = spawn_server().await;

    // Register accounts
    let sender: Value = client
        .post(format!("{base}/api/accounts"))
        .json(&json!({"name": "sender", "display_name": "Sender Agent"}))
        .send().await.unwrap().json().await.unwrap();
    let recipient: Value = client
        .post(format!("{base}/api/accounts"))
        .json(&json!({"name": "recipient"}))
        .send().await.unwrap().json().await.unwrap();

    let sender_token = sender["bearerToken"].as_str().unwrap();
    let recipient_token = recipient["bearerToken"].as_str().unwrap();
    let recipient_id = recipient["id"].as_str().unwrap();

    // Verify account response shape
    assert!(sender["id"].is_string());
    assert_eq!(sender["name"], "sender");
    assert_eq!(sender["displayName"], "Sender Agent");
    assert!(sender["bearerToken"].is_string());

    // Send message
    let sent: Value = client
        .post(format!("{base}/api/messages/send"))
        .bearer_auth(sender_token)
        .json(&json!({
            "to": [recipient_id],
            "subject": "Conformance test",
            "body": "Testing response shapes"
        }))
        .send().await.unwrap().json().await.unwrap();

    // Send response shape: id, threadId, labelIds
    assert!(sent["id"].is_string());
    assert!(sent["threadId"].is_string());
    assert!(sent["labelIds"].is_array());

    let msg_id = sent["id"].as_str().unwrap();
    let thread_id = sent["threadId"].as_str().unwrap();

    // GET message shape
    let msg: Value = client
        .get(format!("{base}/api/messages/{msg_id}"))
        .bearer_auth(recipient_token)
        .send().await.unwrap().json().await.unwrap();

    assert_eq!(msg["id"], msg_id);
    assert_eq!(msg["threadId"], thread_id);
    assert!(msg["from"].is_string());
    assert!(msg["to"].is_array());
    assert!(msg["cc"].is_array());
    assert!(msg["subject"].is_string());
    assert!(msg["body"].is_string());
    assert!(msg["snippet"].is_string());
    assert!(msg["labelIds"].is_array());
    assert!(msg["internalDate"].is_string());
    assert!(msg["hasAttachments"].is_boolean());
    assert!(msg["attachments"].is_array());
    // _diagnostics present on authenticated responses
    assert!(msg["_diagnostics"].is_object());
    assert!(msg["_diagnostics"]["unread_count"].is_number());

    // LIST messages shape
    let list: Value = client
        .get(format!("{base}/api/messages?label=INBOX"))
        .bearer_auth(recipient_token)
        .send().await.unwrap().json().await.unwrap();

    assert!(list["messages"].is_array());
    assert!(list["resultSizeEstimate"].is_number());
    // nextPageToken should be null when no more pages
    assert!(list["nextPageToken"].is_null());
    let list_msg = &list["messages"][0];
    assert!(list_msg["id"].is_string());
    assert!(list_msg["threadId"].is_string());
    assert!(list_msg["snippet"].is_string());
    assert!(list_msg["subject"].is_string());

    // LABELS shape
    let labels: Value = client
        .get(format!("{base}/api/labels"))
        .bearer_auth(recipient_token)
        .send().await.unwrap().json().await.unwrap();

    assert!(labels["labels"].is_array());
    let label = labels["labels"].as_array().unwrap()
        .iter().find(|l| l["name"] == "INBOX").unwrap();
    assert!(label["name"].is_string());
    assert!(label["type"].is_string());
    assert!(label["messagesTotal"].is_number());
    assert!(label["messagesUnread"].is_number());

    // THREADS shape
    let threads: Value = client
        .get(format!("{base}/api/threads?label=INBOX"))
        .bearer_auth(recipient_token)
        .send().await.unwrap().json().await.unwrap();

    assert!(threads["threads"].is_array());
    let thread = &threads["threads"][0];
    assert!(thread["id"].is_string());
    assert!(thread["subject"].is_string());
    assert!(thread["snippet"].is_string());
    assert!(thread["messageCount"].is_number());

    // GET thread shape
    let thread_detail: Value = client
        .get(format!("{base}/api/threads/{thread_id}"))
        .bearer_auth(recipient_token)
        .send().await.unwrap().json().await.unwrap();

    assert!(thread_detail["id"].is_string());
    assert!(thread_detail["messages"].is_array());
    assert!(thread_detail["messageCount"].is_number());

    // Error response shape (404)
    let err_resp = client
        .get(format!("{base}/api/messages/nonexistent"))
        .bearer_auth(recipient_token)
        .send().await.unwrap();
    assert_eq!(err_resp.status(), 404);
    let err: Value = err_resp.json().await.unwrap();
    assert!(err["error"].is_object());
    assert!(err["error"]["code"].is_number());
    assert!(err["error"]["message"].is_string());

    // Auth error shape (401)
    let unauth_resp = client
        .get(format!("{base}/api/messages"))
        .send().await.unwrap();
    assert_eq!(unauth_resp.status(), 401);
}

#[tokio::test]
async fn test_conformance_pagination() {
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

    // Send 5 messages (microsecond timestamp precision ensures unique ordering)
    for i in 0..5 {
        client
            .post(format!("{base}/api/messages/send"))
            .bearer_auth(sender_token)
            .json(&json!({
                "to": [recipient_id],
                "subject": format!("Pagination test {i}"),
                "body": format!("Body {i}")
            }))
            .send().await.unwrap();
    }

    // List with maxResults=2 to trigger pagination
    let page1: Value = client
        .get(format!("{base}/api/messages?label=INBOX&maxResults=2"))
        .bearer_auth(recipient_token)
        .send().await.unwrap().json().await.unwrap();

    assert!(page1["messages"].is_array());
    assert_eq!(page1["messages"].as_array().unwrap().len(), 2);
    assert!(page1["nextPageToken"].is_string(), "should have nextPageToken when more results exist");
    assert!(page1["resultSizeEstimate"].is_number());

    // Fetch page 2
    let token = page1["nextPageToken"].as_str().unwrap();
    let page2: Value = client
        .get(format!("{base}/api/messages?label=INBOX&maxResults=2&pageToken={token}"))
        .bearer_auth(recipient_token)
        .send().await.unwrap().json().await.unwrap();

    assert!(page2["messages"].is_array());
    assert_eq!(page2["messages"].as_array().unwrap().len(), 2);

    // Page 3 should have 1 message
    if let Some(token2) = page2["nextPageToken"].as_str() {
        let page3: Value = client
            .get(format!("{base}/api/messages?label=INBOX&maxResults=2&pageToken={token2}"))
            .bearer_auth(recipient_token)
            .send().await.unwrap().json().await.unwrap();

        assert!(page3["messages"].is_array());
        assert_eq!(page3["messages"].as_array().unwrap().len(), 1);
        // Last page: nextPageToken should be null
        assert!(page3["nextPageToken"].is_null());
    }
}

#[tokio::test]
async fn test_conformance_label_crud_shapes() {
    let (base, client) = spawn_server().await;

    let sender: Value = client
        .post(format!("{base}/api/accounts"))
        .json(&json!({"name": "sender"}))
        .send().await.unwrap().json().await.unwrap();
    let recipient: Value = client
        .post(format!("{base}/api/accounts"))
        .json(&json!({"name": "label-tester"}))
        .send().await.unwrap().json().await.unwrap();
    let sender_token = sender["bearerToken"].as_str().unwrap();
    let token = recipient["bearerToken"].as_str().unwrap();
    let recipient_id = recipient["id"].as_str().unwrap();

    // Send a message so system labels appear in counts
    client
        .post(format!("{base}/api/messages/send"))
        .bearer_auth(sender_token)
        .json(&json!({
            "to": [recipient_id],
            "subject": "Label test",
            "body": "body"
        }))
        .send().await.unwrap();

    // Create label response shape
    let created: Value = client
        .post(format!("{base}/api/labels"))
        .bearer_auth(token)
        .json(&json!({"name": "IMPORTANT"}))
        .send().await.unwrap().json().await.unwrap();

    assert!(created["id"].is_string());
    assert_eq!(created["name"], "IMPORTANT");
    assert_eq!(created["type"], "user");

    // List labels — verify system labels present (need messages for them to show)
    let labels: Value = client
        .get(format!("{base}/api/labels"))
        .bearer_auth(token)
        .send().await.unwrap().json().await.unwrap();

    let labels_arr = labels["labels"].as_array().unwrap();
    let label_names: Vec<&str> = labels_arr.iter()
        .filter_map(|l| l["name"].as_str())
        .collect();
    // INBOX and UNREAD should appear since we received a message
    assert!(label_names.contains(&"INBOX"));
    assert!(label_names.contains(&"UNREAD"));

    // Each label has correct shape
    for label in labels_arr {
        assert!(label["name"].is_string());
        assert!(label["type"].is_string());
        assert!(label["messagesTotal"].is_number());
        assert!(label["messagesUnread"].is_number());
    }

    // Delete label
    let del_resp = client
        .delete(format!("{base}/api/labels/IMPORTANT"))
        .bearer_auth(token)
        .send().await.unwrap();
    assert_eq!(del_resp.status(), 200);
}

#[tokio::test]
async fn test_conformance_modify_shapes() {
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

    let sent: Value = client
        .post(format!("{base}/api/messages/send"))
        .bearer_auth(sender_token)
        .json(&json!({
            "to": [recipient_id],
            "subject": "Modify shape test",
            "body": "body"
        }))
        .send().await.unwrap().json().await.unwrap();
    let msg_id = sent["id"].as_str().unwrap();

    // Modify response shape
    let modified: Value = client
        .post(format!("{base}/api/messages/{msg_id}/modify"))
        .bearer_auth(recipient_token)
        .json(&json!({
            "addLabelIds": ["STARRED"],
            "removeLabelIds": ["UNREAD"]
        }))
        .send().await.unwrap().json().await.unwrap();

    assert_eq!(modified["id"], msg_id);
    assert!(modified["labelIds"].is_array());
    let label_ids: Vec<&str> = modified["labelIds"].as_array().unwrap()
        .iter().filter_map(|v| v.as_str()).collect();
    assert!(label_ids.contains(&"STARRED"));
    assert!(!label_ids.contains(&"UNREAD"));
    assert!(label_ids.contains(&"INBOX"));

    // Trash response shape
    let trashed: Value = client
        .post(format!("{base}/api/messages/{msg_id}/trash"))
        .bearer_auth(recipient_token)
        .send().await.unwrap().json().await.unwrap();

    assert_eq!(trashed["id"], msg_id);
    assert!(trashed["labelIds"].is_array());

    // Batch modify shape
    let batch: Value = client
        .post(format!("{base}/api/messages/batchModify"))
        .bearer_auth(recipient_token)
        .json(&json!({
            "ids": [msg_id],
            "addLabelIds": ["STARRED"]
        }))
        .send().await.unwrap().json().await.unwrap();

    assert!(batch["modified"].is_number());

    // Delete response shape
    let deleted: Value = client
        .delete(format!("{base}/api/messages/{msg_id}"))
        .bearer_auth(recipient_token)
        .send().await.unwrap().json().await.unwrap();

    assert!(deleted["deleted"].is_boolean());
    assert_eq!(deleted["deleted"], true);
}

#[tokio::test]
async fn test_conformance_search_shape() {
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

    client
        .post(format!("{base}/api/messages/send"))
        .bearer_auth(sender_token)
        .json(&json!({
            "to": [recipient_id],
            "subject": "Searchable unique keyword xyzzy",
            "body": "body text"
        }))
        .send().await.unwrap();

    // Search response shape
    let results: Value = client
        .get(format!("{base}/api/search?q=xyzzy"))
        .bearer_auth(recipient_token)
        .send().await.unwrap().json().await.unwrap();

    assert!(results["messages"].is_array());
    assert!(results["resultSizeEstimate"].is_number());
    let msgs = results["messages"].as_array().unwrap();
    assert!(!msgs.is_empty());
    assert!(msgs[0]["id"].is_string());

    // Search with no results
    let empty: Value = client
        .get(format!("{base}/api/search?q=nonexistentkeyword"))
        .bearer_auth(recipient_token)
        .send().await.unwrap().json().await.unwrap();

    assert!(empty["messages"].is_array());
    assert_eq!(empty["messages"].as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn test_update_pane() {
    let (base, client) = spawn_server().await;

    // Register account
    let account: Value = client
        .post(format!("{base}/api/accounts"))
        .json(&json!({"name": "pane-agent"}))
        .send().await.unwrap()
        .json().await.unwrap();
    let token = account["bearerToken"].as_str().unwrap();
    let id = account["id"].as_str().unwrap();

    // Register pane
    let resp = client
        .post(format!("{base}/api/accounts/{id}/pane"))
        .bearer_auth(token)
        .json(&json!({"pane_id": "%42"}))
        .send().await.unwrap();
    assert_eq!(resp.status(), 200);
    let body: Value = resp.json().await.unwrap();
    assert_eq!(body["tmuxPaneId"], "%42");

    // Verify via get account
    let profile: Value = client
        .get(format!("{base}/api/accounts/{id}"))
        .bearer_auth(token)
        .send().await.unwrap()
        .json().await.unwrap();
    assert_eq!(profile["tmuxPaneId"], "%42");

    // Update pane using "me"
    let resp2 = client
        .post(format!("{base}/api/accounts/me/pane"))
        .bearer_auth(token)
        .json(&json!({"pane_id": "%99"}))
        .send().await.unwrap();
    assert_eq!(resp2.status(), 200);
    let body2: Value = resp2.json().await.unwrap();
    assert_eq!(body2["tmuxPaneId"], "%99");
}

#[tokio::test]
async fn test_mailing_list_fanout() {
    let (base, client) = spawn_server().await;

    // Create sender and 3 subscribers
    let sender: Value = client
        .post(format!("{base}/api/accounts"))
        .json(&json!({"name": "sender"}))
        .send().await.unwrap().json().await.unwrap();
    let sub1: Value = client
        .post(format!("{base}/api/accounts"))
        .json(&json!({"name": "sub1"}))
        .send().await.unwrap().json().await.unwrap();
    let sub2: Value = client
        .post(format!("{base}/api/accounts"))
        .json(&json!({"name": "sub2"}))
        .send().await.unwrap().json().await.unwrap();
    let sub3: Value = client
        .post(format!("{base}/api/accounts"))
        .json(&json!({"name": "sub3"}))
        .send().await.unwrap().json().await.unwrap();

    let sender_token = sender["bearerToken"].as_str().unwrap();
    let sub1_token = sub1["bearerToken"].as_str().unwrap();
    let sub2_token = sub2["bearerToken"].as_str().unwrap();
    let sub3_token = sub3["bearerToken"].as_str().unwrap();

    // Create a mailing list and subscribe sub1 + sub2 (not sub3)
    let list: Value = client
        .post(format!("{base}/api/lists"))
        .bearer_auth(sender_token)
        .json(&json!({"name": "team-updates", "description": "Team updates"}))
        .send().await.unwrap().json().await.unwrap();
    let list_id = list["id"].as_str().unwrap();

    client.post(format!("{base}/api/lists/{list_id}/subscribe"))
        .bearer_auth(sender_token)
        .json(&json!({"account_id": sub1["id"].as_str().unwrap()}))
        .send().await.unwrap();
    client.post(format!("{base}/api/lists/{list_id}/subscribe"))
        .bearer_auth(sender_token)
        .json(&json!({"account_id": sub2["id"].as_str().unwrap()}))
        .send().await.unwrap();

    // Send message to the list
    let sent: Value = client
        .post(format!("{base}/api/messages/send"))
        .bearer_auth(sender_token)
        .json(&json!({
            "to": ["list:team-updates"],
            "subject": "Team announcement",
            "body": "Hello everyone"
        }))
        .send().await.unwrap().json().await.unwrap();
    assert!(sent["id"].is_string());

    // sub1 should have the message in INBOX
    let sub1_inbox: Value = client
        .get(format!("{base}/api/messages?label=INBOX"))
        .bearer_auth(sub1_token)
        .send().await.unwrap().json().await.unwrap();
    assert_eq!(sub1_inbox["messages"].as_array().unwrap().len(), 1);
    assert_eq!(sub1_inbox["messages"][0]["subject"], "Team announcement");

    // sub2 should have it too
    let sub2_inbox: Value = client
        .get(format!("{base}/api/messages?label=INBOX"))
        .bearer_auth(sub2_token)
        .send().await.unwrap().json().await.unwrap();
    assert_eq!(sub2_inbox["messages"].as_array().unwrap().len(), 1);

    // sub3 should NOT have it (not subscribed)
    let sub3_inbox: Value = client
        .get(format!("{base}/api/messages?label=INBOX"))
        .bearer_auth(sub3_token)
        .send().await.unwrap().json().await.unwrap();
    assert_eq!(sub3_inbox["messages"].as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn test_send_to_nonexistent_list_rejected() {
    let (base, client) = spawn_server().await;

    let alice: Value = client
        .post(format!("{base}/api/accounts"))
        .json(&json!({"name": "alice"}))
        .send().await.unwrap().json().await.unwrap();
    let token = alice["bearerToken"].as_str().unwrap();

    // Sending to "list:nonexistent" — list doesn't exist, recipient passed through
    // as literal "list:nonexistent" which isn't a valid account ID → FK error
    let resp = client
        .post(format!("{base}/api/messages/send"))
        .bearer_auth(token)
        .json(&json!({
            "to": ["list:nonexistent"],
            "subject": "test",
            "body": "hello"
        }))
        .send().await.unwrap();
    assert!(resp.status().is_server_error() || resp.status().is_client_error());
}

#[tokio::test]
async fn test_thread_reply_chain() {
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
    let alice_id = alice["id"].as_str().unwrap();
    let bob_id = bob["id"].as_str().unwrap();

    // Alice sends original
    let msg1: Value = client
        .post(format!("{base}/api/messages/send"))
        .bearer_auth(alice_token)
        .json(&json!({
            "to": [bob_id],
            "subject": "Hello Bob",
            "body": "How are you?"
        }))
        .send().await.unwrap().json().await.unwrap();
    let msg1_id = msg1["id"].as_str().unwrap();
    let thread_id = msg1["threadId"].as_str().unwrap();

    // Bob replies using in_reply_to
    let msg2: Value = client
        .post(format!("{base}/api/messages/send"))
        .bearer_auth(bob_token)
        .json(&json!({
            "to": [alice_id],
            "subject": "Re: Hello Bob",
            "body": "I'm great!",
            "in_reply_to": msg1_id
        }))
        .send().await.unwrap().json().await.unwrap();

    // Both should be in same thread
    assert_eq!(msg2["threadId"].as_str().unwrap(), thread_id);

    // Thread endpoint should return both messages
    let thread: Value = client
        .get(format!("{base}/api/threads/{thread_id}"))
        .bearer_auth(alice_token)
        .send().await.unwrap().json().await.unwrap();
    let msgs = thread["messages"].as_array().unwrap();
    assert_eq!(msgs.len(), 2);
}

#[tokio::test]
async fn test_diagnostics_unread_count() {
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
    let bob_id = bob["id"].as_str().unwrap();

    // Send 3 messages to Bob
    for i in 0..3 {
        client
            .post(format!("{base}/api/messages/send"))
            .bearer_auth(alice_token)
            .json(&json!({
                "to": [bob_id],
                "subject": format!("msg {i}"),
                "body": "hello"
            }))
            .send().await.unwrap();
    }

    // Bob's diagnostics should show 3 unread
    let inbox: Value = client
        .get(format!("{base}/api/messages?label=INBOX"))
        .bearer_auth(bob_token)
        .send().await.unwrap().json().await.unwrap();
    assert_eq!(inbox["_diagnostics"]["unread_count"], 3);

    // Read one message (GET auto-removes UNREAD)
    let msg_id = inbox["messages"][0]["id"].as_str().unwrap();
    client
        .get(format!("{base}/api/messages/{msg_id}"))
        .bearer_auth(bob_token)
        .send().await.unwrap();

    // Diagnostics should now show 2 unread
    let inbox2: Value = client
        .get(format!("{base}/api/messages?label=INBOX"))
        .bearer_auth(bob_token)
        .send().await.unwrap().json().await.unwrap();
    assert_eq!(inbox2["_diagnostics"]["unread_count"], 2);
}

#[tokio::test]
async fn test_body_compression_roundtrip() {
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
    let bob_id = bob["id"].as_str().unwrap();

    // Create a body > 512 bytes to trigger compression
    let long_body = "A".repeat(1000);

    let sent: Value = client
        .post(format!("{base}/api/messages/send"))
        .bearer_auth(alice_token)
        .json(&json!({
            "to": [bob_id],
            "subject": "Long message",
            "body": long_body
        }))
        .send().await.unwrap().json().await.unwrap();
    let msg_id = sent["id"].as_str().unwrap();

    // Retrieve and verify body is decompressed correctly
    let msg: Value = client
        .get(format!("{base}/api/messages/{msg_id}"))
        .bearer_auth(bob_token)
        .send().await.unwrap().json().await.unwrap();
    assert_eq!(msg["body"].as_str().unwrap(), long_body);
}

#[tokio::test]
async fn test_empty_recipients_rejected() {
    let (base, client) = spawn_server().await;

    let alice: Value = client
        .post(format!("{base}/api/accounts"))
        .json(&json!({"name": "alice"}))
        .send().await.unwrap().json().await.unwrap();
    let token = alice["bearerToken"].as_str().unwrap();

    let resp = client
        .post(format!("{base}/api/messages/send"))
        .bearer_auth(token)
        .json(&json!({
            "to": [],
            "subject": "test",
            "body": "hello"
        }))
        .send().await.unwrap();
    assert_eq!(resp.status(), 400);
}
