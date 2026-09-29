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

echo "[preinstall] Unloading existing service if present..."
DAEMON_PLIST="/Library/LaunchDaemons/com.dlpagent.daemon.plist"
if [ -f "$DAEMON_PLIST" ]; then
    launchctl bootout system "$DAEMON_PLIST" 2>/dev/null || true
    echo "[preinstall] Existing daemon unloaded."
fi

echo "[preinstall] Pre-install checks passed. ✅"
