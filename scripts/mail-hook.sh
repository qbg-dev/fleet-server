#!/bin/bash
# Git post-commit hook for boring-mail notification
# Install: cp scripts/mail-hook.sh .git/hooks/post-commit && chmod +x .git/hooks/post-commit
# Requires: BORING_MAIL_TOKEN env var set

MAIL_SERVER="${BORING_MAIL_SERVER:-http://100.88.146.25:8025}"
TOKEN="${BORING_MAIL_TOKEN:-}"

if [ -z "$TOKEN" ]; then
  exit 0  # No token, skip silently
fi

COMMIT_SHA=$(git rev-parse HEAD)
COMMIT_SHORT=$(git rev-parse --short HEAD)
COMMIT_MSG=$(git log -1 --pretty=%s)
AUTHOR=$(git log -1 --pretty=%an)
BRANCH=$(git branch --show-current)

curl -sf -X POST "$MAIL_SERVER/api/messages/send" \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d "{
    \"to\": [\"all\"],
    \"subject\": \"[$BRANCH] $COMMIT_MSG\",
    \"body\": \"$AUTHOR committed $COMMIT_SHORT on $BRANCH\\n\\n$COMMIT_MSG\",
    \"labels\": [\"COMMIT\"],
    \"source\": \"hook\"
  }" >/dev/null 2>&1 &
