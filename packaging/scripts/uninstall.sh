#!/bin/bash
# uninstall.sh — Full removal of DLP Agent from macOS
#
# Equivalent to: Windows uninstall.exe + sc delete DLPAgent

set -e

DAEMON_PLIST="/Library/LaunchDaemons/com.dlpagent.daemon.plist"
AGENT_PLIST="/Library/LaunchAgents/com.dlpagent.agent.plist"
INSTALL_DIR="/Library/DLPAgent"
DATA_DIR="/Library/Application Support/DLPAgent"
LOG_DIR="/var/log/dlp-agent"

echo "[uninstall] Stopping DLP Agent services..."
launchctl bootout system "$DAEMON_PLIST" 2>/dev/null || true

for USER_HOME in /Users/*/; do
    USERNAME=$(basename "$USER_HOME")
    USER_ID=$(id -u "$USERNAME" 2>/dev/null || true)
    if [ -n "$USER_ID" ] && [ "$USER_ID" -gt 500 ]; then
        launchctl bootout "gui/$USER_ID" "$AGENT_PLIST" 2>/dev/null || true
    fi
done

echo "[uninstall] Removing files..."
rm -f "$DAEMON_PLIST"
rm -f "$AGENT_PLIST"
rm -rf "$INSTALL_DIR"
rm -rf "$DATA_DIR"

echo "[uninstall] DLP Agent removed. ✅"
echo "[uninstall] Note: Log files kept at $LOG_DIR (remove manually if desired)."
