#!/bin/bash
# postinstall.sh — Post-installation script for DLP Agent .pkg (macOS)
#
# Configures system permissions, LaunchDaemon (root), LaunchAgent (user GUI),
# and Enterprise browser policies for Google Chrome, Edge, and Brave.

set -e

INSTALL_DIR="/Library/DLPAgent"
LOG_DIR="/var/log/dlp-agent"
DATA_DIR="/Library/Application Support/DLPAgent"
DAEMON_PLIST="/Library/LaunchDaemons/com.dlpagent.daemon.plist"
AGENT_PLIST="/Library/LaunchAgents/com.dlpagent.agent.plist"

echo "[postinstall] Creating directories..."
mkdir -p "$LOG_DIR"
mkdir -p "$DATA_DIR"
mkdir -p "$INSTALL_DIR"

echo "[postinstall] Setting permissions..."
chown -R root:wheel "$INSTALL_DIR"
chmod 755 "$INSTALL_DIR/dlp-agent" 2>/dev/null || true

# Allow both root daemon and user LaunchAgent to write logs and data
chown -R root:staff "$LOG_DIR"
chmod 777 "$LOG_DIR"
touch "$LOG_DIR/agent.log" "$LOG_DIR/ui-helper.log" "$LOG_DIR/ui-helper-err.log" 2>/dev/null || true
chmod 666 "$LOG_DIR"/*.log 2>/dev/null || true

chown -R root:staff "$DATA_DIR"
chmod 777 "$DATA_DIR"

# Reset any lingering web proxies on all network services to guarantee uninterrupted Safari & browser internet
echo "[postinstall] Ensuring web proxies are disabled..."
for SERVICE in $(networksetup -listallnetworkservices 2>/dev/null | grep -v '\*' | grep -v '^An asterisk' || true); do
    networksetup -setwebproxystate "$SERVICE" off 2>/dev/null || true
    networksetup -setsecurewebproxystate "$SERVICE" off 2>/dev/null || true
done

# ── 1. Enterprise Browser Extension Policy Deployment ──────────────────────────
echo "[postinstall] Configuring Enterprise Browser Policies..."
EXT_DIR="$INSTALL_DIR/chrome_extension"
if [ -d "$EXT_DIR" ]; then
    chmod -R 755 "$EXT_DIR"
fi

# Native Messaging Host Registration for macOS Chrome & Edge
CHROME_NMH_DIR="/Library/Google/Chrome/NativeMessagingHosts"
EDGE_NMH_DIR="/Library/Microsoft/Edge/NativeMessagingHosts"
mkdir -p "$CHROME_NMH_DIR"
mkdir -p "$EDGE_NMH_DIR"

cat <<EOF > "$CHROME_NMH_DIR/com.dlp.agent.json"
{
  "name": "com.dlp.agent",
  "description": "DLP Native Messaging Host",
  "path": "$INSTALL_DIR/dlp-agent",
  "type": "stdio",
  "allowed_origins": [
    "chrome-extension://djjppioopfigijlolbjjehckocofaimh/",
    "chrome-extension://*/*"
  ]
}
EOF
cp "$CHROME_NMH_DIR/com.dlp.agent.json" "$EDGE_NMH_DIR/"

# Auto-Install Browser Extension via External Extensions & Enterprise Forcelist (Chrome, Edge, Brave)
echo "[postinstall] Registering Browser External Extensions for automatic loading..."
CHROME_EXT_DIR="/Library/Application Support/Google/Chrome/External Extensions"
EDGE_EXT_DIR="/Library/Application Support/Microsoft Edge/External Extensions"
BRAVE_EXT_DIR="/Library/Application Support/BraveSoftware/Brave-Browser/External Extensions"

mkdir -p "$CHROME_EXT_DIR" "$EDGE_EXT_DIR" "$BRAVE_EXT_DIR"

cat <<EOF > "$CHROME_EXT_DIR/djjppioopfigijlolbjjehckocofaimh.json"
{
  "external_update_url": "file:///Library/DLPAgent/chrome_extension/update.xml",
  "external_crx": "/Library/DLPAgent/chrome_extension/agent.crx",
  "external_version": "1.0.0"
}
EOF

cp "$CHROME_EXT_DIR/djjppioopfigijlolbjjehckocofaimh.json" "$EDGE_EXT_DIR/" 2>/dev/null || true
cp "$CHROME_EXT_DIR/djjppioopfigijlolbjjehckocofaimh.json" "$BRAVE_EXT_DIR/" 2>/dev/null || true

# Deploy to each user's personal Library
for UDIR in /Users/*; do
    if [ -d "$UDIR/Library" ] && [ "$(basename "$UDIR")" != "Shared" ] && [ "$(basename "$UDIR")" != ".localized" ]; then
        UNAME=$(basename "$UDIR")
        UCHROME="$UDIR/Library/Application Support/Google/Chrome/External Extensions"
        UEDGE="$UDIR/Library/Application Support/Microsoft Edge/External Extensions"
        UBRAVE="$UDIR/Library/Application Support/BraveSoftware/Brave-Browser/External Extensions"
        mkdir -p "$UCHROME" "$UEDGE" "$UBRAVE" 2>/dev/null || true
        cp "$CHROME_EXT_DIR/djjppioopfigijlolbjjehckocofaimh.json" "$UCHROME/" 2>/dev/null || true
        cp "$CHROME_EXT_DIR/djjppioopfigijlolbjjehckocofaimh.json" "$UEDGE/" 2>/dev/null || true
        cp "$CHROME_EXT_DIR/djjppioopfigijlolbjjehckocofaimh.json" "$UBRAVE/" 2>/dev/null || true
        chown -R "$UNAME" "$UCHROME" "$UEDGE" "$UBRAVE" 2>/dev/null || true

        # User-level Enterprise Force-Install Policy
        defaults write "$UDIR/Library/Preferences/com.google.Chrome" ExtensionInstallForcelist -array "djjppioopfigijlolbjjehckocofaimh;file:///Library/DLPAgent/chrome_extension/update.xml" 2>/dev/null || true
        defaults write "$UDIR/Library/Preferences/com.microsoft.Edge" ExtensionInstallForcelist -array "djjppioopfigijlolbjjehckocofaimh;file:///Library/DLPAgent/chrome_extension/update.xml" 2>/dev/null || true
        chown "$UNAME" "$UDIR/Library/Preferences/com.google.Chrome.plist" "$UDIR/Library/Preferences/com.microsoft.Edge.plist" 2>/dev/null || true
    fi
done

# System-level Enterprise Managed Policies (Chrome, Edge)
defaults write /Library/Preferences/com.google.Chrome ExtensionInstallForcelist -array "djjppioopfigijlolbjjehckocofaimh;file:///Library/DLPAgent/chrome_extension/update.xml" 2>/dev/null || true
defaults write /Library/Preferences/com.microsoft.Edge ExtensionInstallForcelist -array "djjppioopfigijlolbjjehckocofaimh;file:///Library/DLPAgent/chrome_extension/update.xml" 2>/dev/null || true
defaults write /Library/Preferences/com.google.Chrome ExtensionInstallAllowlist -array-add "djjppioopfigijlolbjjehckocofaimh" 2>/dev/null || true
defaults write /Library/Preferences/com.microsoft.Edge ExtensionInstallAllowlist -array-add "djjppioopfigijlolbjjehckocofaimh" 2>/dev/null || true

# Managed Preferences directory (recognized as MDM policy by Chromium browsers)
mkdir -p "/Library/Managed Preferences"
defaults write "/Library/Managed Preferences/com.google.Chrome" ExtensionInstallForcelist -array "djjppioopfigijlolbjjehckocofaimh;file:///Library/DLPAgent/chrome_extension/update.xml" 2>/dev/null || true
defaults write "/Library/Managed Preferences/com.microsoft.Edge" ExtensionInstallForcelist -array "djjppioopfigijlolbjjehckocofaimh;file:///Library/DLPAgent/chrome_extension/update.xml" 2>/dev/null || true

# ── 2. Loading Services ───────────────────────────────────────────────────────
echo "[postinstall] Loading LaunchDaemon (root service)..."
launchctl bootout system "$DAEMON_PLIST" 2>/dev/null || true
launchctl bootstrap system "$DAEMON_PLIST"
launchctl kickstart -k system/com.dlpagent.daemon 2>/dev/null || true
echo "[postinstall] ✅ LaunchDaemon loaded."

echo "[postinstall] Loading LaunchAgent (user GUI helper)..."
CONSOLE_USER=$(echo "show State:/Users/ConsoleUser" | scutil 2>/dev/null | awk '/Name :/ && !/loginwindow/ { print $3 }')
if [ -z "$CONSOLE_USER" ]; then
    CONSOLE_USER=$(stat -f "%Su" /dev/console 2>/dev/null || echo "")
fi
CONSOLE_UID=$(id -u "$CONSOLE_USER" 2>/dev/null || echo "")

for USER_HOME in /Users/*/; do
    USERNAME=$(basename "$USER_HOME")
    if [ "$USERNAME" = "Shared" ] || [ "$USERNAME" = ".localized" ]; then
        continue
    fi
    USER_ID=$(id -u "$USERNAME" 2>/dev/null || true)
    if [ -n "$USER_ID" ] && [ "$USER_ID" -gt 500 ]; then
        launchctl bootout "gui/$USER_ID/com.dlpagent.agent" 2>/dev/null || true
        launchctl bootstrap "gui/$USER_ID" "$AGENT_PLIST" 2>/dev/null || true
        launchctl enable "gui/$USER_ID/com.dlpagent.agent" 2>/dev/null || true
        launchctl kickstart -k -p "gui/$USER_ID/com.dlpagent.agent" 2>/dev/null || true
        echo "[postinstall] ✅ LaunchAgent loaded for: $USERNAME (uid=$USER_ID)"
    fi
done

# ── 3. Auto-Install Privacy Profile (if present) ──────────────────────────────
if [ -f "$INSTALL_DIR/dlp-agent-privacy.mobileconfig" ]; then
    echo "[postinstall] Attempting silent MDM Privacy Profile registration..."
    /usr/bin/profiles install -path "$INSTALL_DIR/dlp-agent-privacy.mobileconfig" 2>/dev/null || true
fi

# ── 4. Auto-Launch Setup & Activation Wizard ──────────────────────────────────
echo "[postinstall] Launching Setup Wizard in active user GUI session..."
TARGET_USER="$CONSOLE_USER"
TARGET_UID="$CONSOLE_UID"

if [ -z "$TARGET_USER" ] || [ "$TARGET_USER" = "root" ] || [ -z "$TARGET_UID" ] || [ "$TARGET_UID" -le 500 ]; then
    for UDIR in /Users/*; do
        UNAME=$(basename "$UDIR")
        if [ "$UNAME" != "Shared" ] && [ "$UNAME" != ".localized" ]; then
            UID_C=$(id -u "$UNAME" 2>/dev/null || echo 0)
            if [ "$UID_C" -gt 500 ]; then
                TARGET_USER="$UNAME"
                TARGET_UID="$UID_C"
                break
            fi
        fi
    done
fi

if [ -n "$TARGET_UID" ] && [ "$TARGET_UID" -gt 500 ]; then
    echo "[postinstall] Launching setup wizard for user $TARGET_USER (uid $TARGET_UID)..."
    launchctl asuser "$TARGET_UID" /Library/DLPAgent/dlp-agent --setup >> "$LOG_DIR/setup.log" 2>&1 &
fi

echo "[postinstall] DLP Agent installation complete! ✅"

