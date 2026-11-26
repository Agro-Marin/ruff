#!/bin/bash
set -euo pipefail

if [ "${CLAUDE_CODE_REMOTE:-}" = "true" ]; then
  exec "$(dirname "$0")/session-start-web.sh"
fi
