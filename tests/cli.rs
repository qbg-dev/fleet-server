/// CLI subcommand tests for boring-mail binary.
use std::process::Command;

fn boring_mail() -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_boring-mail"));
    let tmp = tempfile::tempdir().unwrap();
    cmd.env("BORING_MAIL_DATA_DIR", tmp.path().to_str().unwrap());
    // Leak the tempdir so it stays alive for the command
    std::mem::forget(tmp);
    cmd
}

#[test]
fn test_cli_help() {
    let output = boring_mail().arg("--help").output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("Gmail-conformant mail server"));
    assert!(stdout.contains("serve"));
    assert!(stdout.contains("init"));
    assert!(stdout.contains("status"));
    assert!(stdout.contains("accounts"));
}

#[test]
fn test_cli_init() {
    let tmp = tempfile::tempdir().unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_boring-mail"))
        .arg("init")
        .env("BORING_MAIL_DATA_DIR", tmp.path().to_str().unwrap())
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("Initializing boring-mail"));
    assert!(stdout.contains("Ready."));
    assert!(tmp.path().join("mail.db").exists());
    assert!(tmp.path().join("blobs").is_dir());
}

#[test]
fn test_cli_status() {
    let tmp = tempfile::tempdir().unwrap();
    // Initialize first
    Command::new(env!("CARGO_BIN_EXE_boring-mail"))
        .arg("init")
        .env("BORING_MAIL_DATA_DIR", tmp.path().to_str().unwrap())
        .output()
        .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_boring-mail"))
        .arg("status")
        .env("BORING_MAIL_DATA_DIR", tmp.path().to_str().unwrap())
        // Use a port unlikely to be in use so status reports "not running"
        .env("BORING_MAIL_BIND", "127.0.0.1:19999")
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("boring-mail status"));
    assert!(stdout.contains("Accounts: 0"));
    assert!(stdout.contains("Messages: 0"));
    assert!(stdout.contains("Server:   not running"));
}

#[test]
fn test_cli_accounts_empty() {
    let tmp = tempfile::tempdir().unwrap();
    Command::new(env!("CARGO_BIN_EXE_boring-mail"))
        .arg("init")
        .env("BORING_MAIL_DATA_DIR", tmp.path().to_str().unwrap())
        .output()
        .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_boring-mail"))
        .arg("accounts")
        .env("BORING_MAIL_DATA_DIR", tmp.path().to_str().unwrap())
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("No accounts registered."));
}
