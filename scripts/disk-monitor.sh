#!/bin/bash
# Disk monitor for boring_mail_server on kevinster
# Runs in bms:monitor window, checks every 15 minutes

while true; do
  echo "=== $(date) ==="
  df -h /
  du -sh ~/boring_mail_server/target 2>/dev/null || echo "no target/ yet"

  # Auto-clean if disk > 70%
  USAGE=$(df / | tail -1 | awk '{print $5}' | tr -d '%')
  if [ "$USAGE" -gt 70 ]; then
    echo "DISK HIGH ($USAGE%) — cleaning build artifacts"
    cargo clean --manifest-path ~/boring_mail_server/Cargo.toml 2>/dev/null
    find ~/references -name target -type d -exec rm -rf {} + 2>/dev/null
  fi
  sleep 900
done
