#!/bin/bash
# Git post-commit hook for boring-mail notification
# Install: cp scripts/mail-hook.sh .git/hooks/post-commit && chmod +x .git/hooks/post-commit
# Config: BORING_MAIL_SERVER, BORING_MAIL_RECIPIENTS (comma-separated account IDs)

MAIL_SERVER="${BORING_MAIL_SERVER:-http://100.88.146.25:8025}"
RECIPIENTS="${BORING_MAIL_RECIPIENTS:-}"

if [ -z "$RECIPIENTS" ]; then
  exit 0  # No recipients configured, skip
fi

COMMIT_SHA=$(git rev-parse HEAD)
COMMIT_MSG=$(git log -1 --pretty=%s)
AUTHOR=$(git log -1 --pretty=%an)

# Convert comma-separated to JSON array
IFS=',' read -ra ADDR <<< "$RECIPIENTS"
JSON_RECIPIENTS=$(printf '"%s",' "${ADDR[@]}")
JSON_RECIPIENTS="[${JSON_RECIPIENTS%,}]"

curl -sf -X POST "$MAIL_SERVER/api/webhooks/git-commit" \
  -H "Content-Type: application/json" \
  -d "{
    \"author\": \"$AUTHOR\",
    \"sha\": \"$COMMIT_SHA\",
    \"message\": \"$COMMIT_MSG\",
    \"recipients\": $JSON_RECIPIENTS
  }" >/dev/null 2>&1 &
