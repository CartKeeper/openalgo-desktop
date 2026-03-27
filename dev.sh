#!/usr/bin/env bash
# OpenAlgo Desktop — dev launcher
# Starts all three processes needed for local development:
#   1. openalgo backend via uv
#   2. openalgo backend via python3.11
#   3. Tauri dev (frontend + native shell)

cleanup() {
  echo "Shutting down all processes..."
  kill 0 2>/dev/null
  wait 2>/dev/null
}
trap cleanup EXIT INT TERM

# 1. OpenAlgo backend (uv)
(cd /Users/jasonborst/openalgo && uv run app.py) &

# 2. OpenAlgo backend (python3.11)
(cd /Users/jasonborst/openalgo && /usr/local/bin/python3.11 app.py) &

# 3. Tauri dev
cd /Users/jasonborst/openalgo-desktop && npx tauri dev

# If tauri exits, the trap will clean up the backgrounded processes
