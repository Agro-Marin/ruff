#!/bin/bash
set -euo pipefail

if ! command -v gh &> /dev/null; then
    apt-get update -qq
    apt-get install -y -qq gh
fi

if [ -n "${CLAUDE_ENV_FILE:-}" ]; then
  echo 'export GH_REPO=astral-sh/ruff' >> "$CLAUDE_ENV_FILE"
fi

rustup component add clippy rustfmt

echo "nextest is not installed, so just use 'cargo test' instead."
