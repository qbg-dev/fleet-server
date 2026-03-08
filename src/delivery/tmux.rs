// tmux push notification delivery — Phase 5
// Sends a one-liner notification to the recipient's registered tmux pane.

use std::process::Command;

/// Check if a tmux pane is alive.
pub fn pane_alive(pane_id: &str) -> bool {
    Command::new("tmux")
        .args(["has-session", "-t", pane_id])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Send a notification to a tmux pane by writing to its input.
/// Uses `tmux display-message` targeted at the pane for non-intrusive notification.
pub fn notify_pane(pane_id: &str, message: &str) -> bool {
    // Use tmux set-buffer + paste to avoid interfering with running processes.
    // Instead, display a tmux message bar on the pane's window.
    let msg = format!("[boring-mail] {message}");

    // tmux display-message only shows on the active window, so we use
    // send-keys with a comment prefix to be safe for shell panes,
    // but display-message -t is better for non-intrusive alerts.
    Command::new("tmux")
        .args([
            "display-message",
            "-t",
            pane_id,
            "-d",
            "5000", // display for 5 seconds
            &msg,
        ])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Batch notification: summarize multiple new messages into one notification.
pub fn notify_new_messages(pane_id: &str, count: usize, from: &str, subject: &str) {
    if !pane_alive(pane_id) {
        tracing::debug!("pane {pane_id} is dead, skipping notification");
        return;
    }

    let msg = if count == 1 {
        format!("New mail from {from}: {}", truncate(subject, 60))
    } else {
        format!("{count} new messages (latest from {from}: {})", truncate(subject, 40))
    };

    if !notify_pane(pane_id, &msg) {
        tracing::warn!("failed to notify pane {pane_id}");
    }
}

fn truncate(s: &str, max: usize) -> &str {
    if s.len() <= max {
        s
    } else {
        let end = s.floor_char_boundary(max.saturating_sub(3));
        // Caller should append "..." if desired; we just return the slice
        &s[..end]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_truncate() {
        assert_eq!(truncate("hello", 10), "hello");
        assert_eq!(truncate("hello world this is long", 10), "hello w");
    }

    #[test]
    fn test_pane_alive_nonexistent() {
        // A pane that doesn't exist should return false
        assert!(!pane_alive("%99999"));
    }
}
