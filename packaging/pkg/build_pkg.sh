#!/bin/bash
# build_pkg.sh — Build a Professional macOS Branded .pkg Installer for DLP Agent
#
# Generates:
#   1. core.pkg (Payload component containing /Library/DLPAgent, LaunchDaemons, etc.)
#   2. DLPAgent.pkg (Final branded Apple Distribution installer with Welcome, License, and Conclusion UI)
#
# Requires macOS with Xcode Command Line Tools.

set -e

VERSION="1.0.0"
IDENTIFIER="com.dlpagent.core"
STAGING="./staging"
COMPONENT_PKG="./core.pkg"
FINAL_PKG="./DLPAgent-${VERSION}.pkg"

echo "============================================================"
echo "🍎 BUILDING macOS DLP AGENT PROFESSIONAL INSTALLER"
echo "============================================================"

# ── 1. Build Universal Binary (Apple Silicon arm64 + Intel x86_64) ────────────
echo "🔨 [1/5] Compiling targets for arm64 and x86_64..."
cargo build --release --target aarch64-apple-darwin
cargo build --release --target x86_64-apple-darwin

echo "🔗 [2/5] Creating Universal Binary with lipo..."
mkdir -p "$STAGING/Library/DLPAgent"
lipo -create \
  target/aarch64-apple-darwin/release/dlp-agent \
  target/x86_64-apple-darwin/release/dlp-agent \
  -output "$STAGING/Library/DLPAgent/dlp-agent"

chmod 755 "$STAGING/Library/DLPAgent/dlp-agent"

# ── 2. Stage Supporting Assets ────────────────────────────────────────────────
echo "📦 [3/5] Staging files, extensions, and LaunchDaemons..."
mkdir -p "$STAGING/Library/LaunchDaemons"
mkdir -p "$STAGING/Library/LaunchAgents"

cp -r chrome_extension "$STAGING/Library/DLPAgent/" 2>/dev/null || true
cp packaging/plist/com.dlpagent.daemon.plist "$STAGING/Library/LaunchDaemons/"
cp packaging/plist/com.dlpagent.agent.plist "$STAGING/Library/LaunchAgents/"
cp packaging/pkg/resources/dlp-agent-privacy.mobileconfig "$STAGING/Library/DLPAgent/" 2>/dev/null || true

# Code signing if developer identity is available
if [ -n "$DEVELOPER_ID" ]; then
    echo "🔏 Signing binary with Developer ID..."
    codesign --force --deep --sign "Developer ID Application: $DEVELOPER_ID" \
        --entitlements platform/macos/entitlements.plist \
        --options runtime \
        "$STAGING/Library/DLPAgent/dlp-agent"
fi

# ── 3. Build Component Package ────────────────────────────────────────────────
echo "📦 [4/5] Building component package (pkgbuild)..."
pkgbuild \
    --root "$STAGING" \
    --identifier "$IDENTIFIER" \
    --version "$VERSION" \
    --scripts packaging/scripts \
    --install-location / \
    "$COMPONENT_PKG"

# ── 4. Build Final Branded Distribution Package ───────────────────────────────
echo "🎨 [5/5] Building Branded Apple Installer Wizard (productbuild)..."
productbuild \
    --distribution packaging/pkg/Distribution.xml \
    --resources packaging/pkg/resources \
    --package-path . \
    "$FINAL_PKG"

# Cleanup intermediate files
rm -f "$COMPONENT_PKG"
rm -rf "$STAGING"

echo ""
echo "============================================================"
echo "✅ BUILD COMPLETE! 🎉"
echo "Generated Professional Installer: $FINAL_PKG"
echo "============================================================"
