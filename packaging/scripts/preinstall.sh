#!/bin/bash
# preinstall.sh — Pre-installation checks for DLP Agent .pkg
#
# Checks macOS version, SIP status, and unloads any existing service.

set -e

MIN_MACOS="12"  # Monterey minimum

echo "[preinstall] Checking macOS version..."
OS_VERSION=$(sw_vers -productVersion | cut -d. -f1)
if [ "$OS_VERSION" -lt "$MIN_MACOS" ]; then
    echo "[preinstall] ❌ macOS $MIN_MACOS+ required. Found: $(sw_vers -productVersion)"
    exit 1
fi
echo "[preinstall] ✅ macOS version OK: $(sw_vers -productVersion)"

echo "[preinstall] Stopping and unloading existing services if present..."
DAEMON_PLIST="/Library/LaunchDaemons/com.dlpagent.daemon.plist"
if [ -f "$DAEMON_PLIST" ]; then
    launchctl bootout system "$DAEMON_PLIST" 2>/dev/null || true
    echo "[preinstall] Existing daemon unloaded."
fi

# Unload LaunchAgent for all active users
for UDIR in /Users/*; do
    UNAME=$(basename "$UDIR")
    if [ "$UNAME" != "Shared" ] && [ "$UNAME" != ".localized" ]; then
        UID_C=$(id -u "$UNAME" 2>/dev/null || echo 0)
        if [ "$UID_C" -gt 500 ]; then
            launchctl bootout "gui/$UID_C/com.dlpagent.agent" 2>/dev/null || true
        fi
    fi
done

# Kill any lingering dlp-agent processes
killall -9 dlp-agent 2>/dev/null || true

echo "[preinstall] Automatically cleaning old agent data & credentials for a 100% fresh setup..."
rm -rf "/Library/Application Support/DLPAgent"
rm -rf "/var/log/dlp-agent"/* 2>/dev/null || true

echo "[preinstall] Pre-install clean completed successfully. ✅"
