/// CLI subcommand tests for boring-mail binary.
use std::process::Command;

fn boring_mail() -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_boring-mail"));
    let tmp = tempfile::tempdir().unwrap();
    let base_url = std::env::var("BORING_MAIL_TEST_DB_BASE")
        .unwrap_or_else(|_| "mysql://root@localhost:3307".to_string());
    let db_name = format!("test_cli_{}", uuid::Uuid::new_v4().simple());
    cmd.env("BORING_MAIL_DATA_DIR", tmp.path().to_str().unwrap());
    cmd.env("BORING_MAIL_DATABASE_URL", format!("{base_url}/{db_name}"));
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
    let output = boring_mail().arg("init").output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("Initializing boring-mail"));
    assert!(stdout.contains("Ready."));
}

#[test]
fn test_cli_status() {
    // Init first, then check status
    let base_url = std::env::var("BORING_MAIL_TEST_DB_BASE")
        .unwrap_or_else(|_| "mysql://root@localhost:3307".to_string());
    let db_name = format!("test_cli_{}", uuid::Uuid::new_v4().simple());
    let db_url = format!("{base_url}/{db_name}");
    let tmp = tempfile::tempdir().unwrap();

    Command::new(env!("CARGO_BIN_EXE_boring-mail"))
        .arg("init")
        .env("BORING_MAIL_DATA_DIR", tmp.path().to_str().unwrap())
        .env("BORING_MAIL_DATABASE_URL", &db_url)
        .output()
        .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_boring-mail"))
        .arg("status")
        .env("BORING_MAIL_DATA_DIR", tmp.path().to_str().unwrap())
        .env("BORING_MAIL_DATABASE_URL", &db_url)
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
    let base_url = std::env::var("BORING_MAIL_TEST_DB_BASE")
        .unwrap_or_else(|_| "mysql://root@localhost:3307".to_string());
    let db_name = format!("test_cli_{}", uuid::Uuid::new_v4().simple());
    let db_url = format!("{base_url}/{db_name}");
    let tmp = tempfile::tempdir().unwrap();

    Command::new(env!("CARGO_BIN_EXE_boring-mail"))
        .arg("init")
        .env("BORING_MAIL_DATA_DIR", tmp.path().to_str().unwrap())
        .env("BORING_MAIL_DATABASE_URL", &db_url)
        .output()
        .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_boring-mail"))
        .arg("accounts")
        .env("BORING_MAIL_DATA_DIR", tmp.path().to_str().unwrap())
        .env("BORING_MAIL_DATABASE_URL", &db_url)
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("No accounts registered."));
}
