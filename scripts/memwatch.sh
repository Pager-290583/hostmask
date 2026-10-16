#!/bin/sh
# Samples the memory of a running Hostmask process into a CSV file.
#
# Usage: scripts/memwatch.sh [seconds-between-samples] [output.csv]
#   Default: one sample every 10 seconds, written to memwatch.csv in the current folder.
#   Stop with Ctrl+C.
#
# Use it while you exercise the app: open and close the window, switch sets from the tray,
# activate sets. If the memory keeps growing after the same actions are repeated and the
# app goes back to idle, that is a leak. A single increase after the first activation is
# normal (caches, window rendering).
set -eu

INTERVAL=${1:-10}
OUT=${2:-memwatch.csv}

PID=$(pgrep -f "Hostmask.app/Contents/MacOS/Hostmask|hostmask-gui" | head -1 || true)
if [ -z "$PID" ]; then
    echo "Hostmask is not running" >&2
    exit 1
fi

echo "time,rss_kb,footprint" > "$OUT"
echo "Watching pid $PID every ${INTERVAL}s -> $OUT (Ctrl+C to stop)"
while kill -0 "$PID" 2>/dev/null; do
    RSS=$(ps -o rss= -p "$PID" | tr -d ' ')
    # `footprint` is macOS's own measure; it matches Activity Monitor's "Memory" column.
    FOOT=$(footprint -p "$PID" 2>/dev/null | sed -n 's/.*Footprint: \([0-9.]* [A-Za-z]*\).*/\1/p' | head -1)
    echo "$(date +%H:%M:%S),$RSS,${FOOT:-}" | tee -a "$OUT"
    sleep "$INTERVAL"
done
echo "Process $PID exited"
