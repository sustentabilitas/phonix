#!/usr/bin/env bash
# Show a Recall.ai bot's status history (diagnose lobby / join issues).
#
# Usage: scripts/recall/status.sh <bot_id>
# Env:   RECALL_API_KEY (required), RECALL_REGION (default us-west-2)
set -euo pipefail

[ $# -ge 1 ] || { echo "usage: status.sh <bot_id>" >&2; exit 2; }
: "${RECALL_API_KEY:?set RECALL_API_KEY}"
command -v jq >/dev/null || { echo "jq is required" >&2; exit 1; }
region=${RECALL_REGION:-us-west-2}

curl -sS -H "Authorization: Token $RECALL_API_KEY" \
  "https://${region}.recall.ai/api/v1/bot/$1" | jq '.status_changes'
