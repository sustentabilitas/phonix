#!/usr/bin/env bash
# Make a Recall.ai bot leave its call.
#
# Usage: scripts/recall/leave.sh <bot_id>
# Env:   RECALL_API_KEY (required), RECALL_REGION (default us-west-2)
set -euo pipefail

[ $# -ge 1 ] || { echo "usage: leave.sh <bot_id>" >&2; exit 2; }
: "${RECALL_API_KEY:?set RECALL_API_KEY}"
region=${RECALL_REGION:-us-west-2}

curl -sS -X POST "https://${region}.recall.ai/api/v1/bot/$1/leave_call" \
  -H "Authorization: Token $RECALL_API_KEY" >/dev/null
echo "requested leave for bot $1"
