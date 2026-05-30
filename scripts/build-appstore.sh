#!/usr/bin/env bash
set -euo pipefail

# ============================================================
# PhotoCurate Mac App Store Build Script
# ============================================================
#
# Prerequisites (one-time setup):
#   1. Enroll in Apple Developer Program ($99/year)
#      https://developer.apple.com/programs/
#
#   2. Create an "Apple Distribution" signing certificate:
#      https://developer.apple.com/account/resources/certificates
#      → Certificates → + → "Apple Distribution"
#
#   3. Install the certificate in your Keychain:
#      Double-click the downloaded .cer file
#
#   4. Create an App ID in Identifiers:
#      https://developer.apple.com/account/resources/identifiers
#      → + → "App IDs" → Bundle ID: com.photocurate
#
#   5. Create an App Store Connect app record:
#      https://appstoreconnect.apple.com/
#      → My Apps → + → New App → Bundle ID: com.photocurate
#
#   6. Create a Mac App Store provisioning profile:
#      https://developer.apple.com/account/resources/profiles
#      → + → "Mac App Store" → Select App ID → Select Certificate
#
# Usage:
#   export APPLE_SIGNING_IDENTITY="Apple Distribution: Aaron Lou (V63B559WYX)"
#   export APPLE_INSTALLER_IDENTITY="3rd Party Mac Developer Installer: Aaron Lou (V63B559WYX)"
#   export APPLE_PROVIDER_SHORT_NAME="V63B559WYX"
#   bash scripts/build-appstore.sh
#
# ============================================================

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

echo -e "${GREEN}=== PhotoCurate Mac App Store Build ===${NC}\n"

# --- Validate environment ---
check_var() {
    if [ -z "${!1:-}" ]; then
        echo -e "${RED}ERROR: $1 is not set.${NC}"
        echo "  export $1=\"...\""
        exit 1
    fi
}

check_var APPLE_SIGNING_IDENTITY
check_var APPLE_INSTALLER_IDENTITY
check_var APPLE_PROVIDER_SHORT_NAME

# --- Verify certificates exist in Keychain ---
echo -e "${YELLOW}Verifying signing certificate...${NC}"
if security find-identity -v -p macappstore -s "$APPLE_SIGNING_IDENTITY" 2>/dev/null | grep -q "$APPLE_SIGNING_IDENTITY"; then
    echo -e "  ${GREEN}Found:${NC} $APPLE_SIGNING_IDENTITY"
else
    echo -e "${RED}Certificate not found in Keychain:${NC} $APPLE_SIGNING_IDENTITY"
    echo ""
    echo "  Available 'Apple Distribution' certificates in your Keychain:"
    security find-identity -v -p macappstore 2>/dev/null || echo "  (none)"
    echo ""
    echo "  Make sure you have created and installed an 'Apple Distribution' certificate."
    echo "  See prerequisites in this script's header comments."
    exit 1
fi

echo -e "${YELLOW}Verifying installer certificate...${NC}"
if security find-identity -v -p basic -s "$APPLE_INSTALLER_IDENTITY" 2>/dev/null | grep -q "$APPLE_INSTALLER_IDENTITY"; then
    echo -e "  ${GREEN}Found:${NC} $APPLE_INSTALLER_IDENTITY"
else
    echo -e "${RED}Installer certificate not found in Keychain:${NC} $APPLE_INSTALLER_IDENTITY"
    echo ""
    echo "  Available signing identities in your Keychain:"
    security find-identity -v -p basic 2>/dev/null || echo "  (none)"
    exit 1
fi

echo -e "${YELLOW}Validating release metadata...${NC}"
plutil -lint src-tauri/Entitlements.plist >/dev/null
plutil -lint src-tauri/PrivacyInfo.xcprivacy >/dev/null
if grep -q 'shell:' src-tauri/capabilities/default.json; then
    echo -e "${RED}Refusing App Store build: shell capability is enabled.${NC}"
    exit 1
fi
if grep -q 'fs:allow-home-read\|fs:scope-home' src-tauri/capabilities/default.json src-tauri/tauri.conf.json; then
    echo -e "${RED}Refusing App Store build: broad home-directory access is enabled.${NC}"
    exit 1
fi
echo -e "  ${GREEN}Metadata OK${NC}"

# App Store builds must use App Store delivery for updates. The regular GitHub
# DMG build keeps Tauri updater enabled; this script temporarily strips updater
# config and permissions while the App Store package is produced.
TAURI_CONF_BACKUP="$(mktemp)"
CAPABILITY_BACKUP="$(mktemp)"
GENERATED_CAPABILITY_BACKUP="$(mktemp)"
cp src-tauri/tauri.conf.json "$TAURI_CONF_BACKUP"
cp src-tauri/capabilities/default.json "$CAPABILITY_BACKUP"
cp src-tauri/gen/schemas/capabilities.json "$GENERATED_CAPABILITY_BACKUP"

restore_distribution_files() {
    cp "$TAURI_CONF_BACKUP" src-tauri/tauri.conf.json
    cp "$CAPABILITY_BACKUP" src-tauri/capabilities/default.json
    cp "$GENERATED_CAPABILITY_BACKUP" src-tauri/gen/schemas/capabilities.json
}
trap restore_distribution_files EXIT

echo -e "${YELLOW}Preparing App Store distribution config...${NC}"
node --input-type=commonjs <<'NODE'
const fs = require("fs");

const tauriConfigPath = "src-tauri/tauri.conf.json";
const tauriConfig = JSON.parse(fs.readFileSync(tauriConfigPath, "utf8"));
if (tauriConfig.bundle) {
  tauriConfig.bundle.createUpdaterArtifacts = false;
}
if (tauriConfig.plugins) {
  delete tauriConfig.plugins.updater;
  if (Object.keys(tauriConfig.plugins).length === 0) {
    delete tauriConfig.plugins;
  }
}
fs.writeFileSync(tauriConfigPath, `${JSON.stringify(tauriConfig, null, 2)}\n`);

const capabilityPath = "src-tauri/capabilities/default.json";
const capability = JSON.parse(fs.readFileSync(capabilityPath, "utf8"));
capability.permissions = capability.permissions.filter(
  (permission) => typeof permission !== "string" || (
    !permission.startsWith("updater:") &&
    !permission.startsWith("process:")
  )
);
fs.writeFileSync(capabilityPath, `${JSON.stringify(capability, null, 2)}\n`);
NODE
if grep -q 'updater:' src-tauri/capabilities/default.json src-tauri/tauri.conf.json; then
    echo -e "${RED}Refusing App Store build: updater is still enabled.${NC}"
    exit 1
fi
echo -e "  ${GREEN}Updater disabled for App Store build${NC}"

# --- Verify provisioning profile exists ---
echo -e "${YELLOW}Checking provisioning profiles...${NC}"
PROFILE_COUNT=$(ls ~/Library/MobileDevice/Provisioning\ Profiles/*.provisionprofile 2>/dev/null | wc -l | tr -d ' ')
if [ "$PROFILE_COUNT" -eq 0 ]; then
    echo -e "  ${YELLOW}WARNING: No provisioning profiles found.${NC}"
    echo "  Download your Mac App Store provisioning profile from:"
    echo "  https://developer.apple.com/account/resources/profiles"
else
    echo -e "  ${GREEN}Found $PROFILE_COUNT provisioning profile(s)${NC}"
fi

# --- Verify frontend is built ---
echo -e "${YELLOW}Building frontend...${NC}"
cd "$(dirname "$0")/.."
export VITE_APP_STORE=true
npm run build

# --- Build the Tauri app with App Store signing ---
echo -e "\n${YELLOW}Building .app bundle (Mac App Store signed)...${NC}"

export APPLE_SIGNING_IDENTITY
export APPLE_PROVIDER_SHORT_NAME

npm run tauri-build -- --bundles app --features app-store

APP_PATH="src-tauri/target/release/bundle/macos/PhotoCurate.app"
PROFILE_PATH="$HOME/Library/MobileDevice/Provisioning Profiles/com.photocurate.provisionprofile"
PROFILE_PLIST="src-tauri/target/release/bundle/macos/profile.plist"
APPSTORE_ENTITLEMENTS="src-tauri/target/release/bundle/macos/AppStoreEntitlements.plist"

# --- Embed provisioning profile ---
echo -e "\n${YELLOW}Embedding provisioning profile...${NC}"
if [ -f "$PROFILE_PATH" ]; then
    cp "$PROFILE_PATH" "$APP_PATH/Contents/embedded.provisionprofile"
    xattr -cr "$APP_PATH/Contents/embedded.provisionprofile" 2>/dev/null || true
    echo -e "  ${GREEN}Provisioning profile embedded${NC}"
else
    echo -e "  ${RED}Provisioning profile not found at $PROFILE_PATH${NC}"
    exit 1
fi

echo -e "\n${YELLOW}Generating App Store entitlements...${NC}"
openssl cms -verify -inform DER -in "$PROFILE_PATH" -noverify -out "$PROFILE_PLIST" >/dev/null 2>&1
APP_IDENTIFIER=$(/usr/libexec/PlistBuddy -c "Print :Entitlements:com.apple.application-identifier" "$PROFILE_PLIST")
TEAM_IDENTIFIER=$(/usr/libexec/PlistBuddy -c "Print :Entitlements:com.apple.developer.team-identifier" "$PROFILE_PLIST")
KEYCHAIN_GROUP=$(/usr/libexec/PlistBuddy -c "Print :Entitlements:keychain-access-groups:0" "$PROFILE_PLIST")

cat > "$APPSTORE_ENTITLEMENTS" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>com.apple.application-identifier</key>
    <string>$APP_IDENTIFIER</string>
    <key>com.apple.developer.team-identifier</key>
    <string>$TEAM_IDENTIFIER</string>
    <key>keychain-access-groups</key>
    <array>
        <string>$KEYCHAIN_GROUP</string>
    </array>
    <key>com.apple.security.app-sandbox</key>
    <true/>
    <key>com.apple.security.files.user-selected.read-write</key>
    <true/>
    <key>com.apple.security.files.bookmarks.app-scope</key>
    <true/>
    <key>com.apple.security.network.client</key>
    <true/>
</dict>
</plist>
EOF
plutil -lint "$APPSTORE_ENTITLEMENTS" >/dev/null
echo -e "  ${GREEN}Entitlements generated for:${NC} $APP_IDENTIFIER"

# --- Remove browser/download quarantine attributes before signing ---
echo -e "\n${YELLOW}Removing extended attributes...${NC}"
xattr -cr "$APP_PATH" 2>/dev/null || true
if xattr -lr "$APP_PATH" 2>/dev/null | grep -q "com.apple.quarantine"; then
    echo -e "${RED}Quarantine attribute still present in app bundle.${NC}"
    exit 1
fi
echo -e "  ${GREEN}Extended attributes cleaned${NC}"

# --- Re-sign after embedding profile ---
echo -e "\n${YELLOW}Re-signing with embedded provisioning profile...${NC}"
codesign --sign "$APPLE_SIGNING_IDENTITY" \
    --entitlements "$APPSTORE_ENTITLEMENTS" \
    --options runtime \
    --deep \
    --force \
    "$APP_PATH" 2>&1
echo -e "  ${GREEN}Re-signed${NC}"

# --- Verify signed bundle ---
echo -e "\n${YELLOW}Verifying signed .app...${NC}"
codesign --verify --deep --strict --verbose=2 "$APP_PATH"
codesign --display --entitlements :- "$APP_PATH" | grep -q "com.apple.security.app-sandbox"
codesign --display --entitlements :- "$APP_PATH" | grep -q "com.apple.application-identifier"
test -f "$APP_PATH/Contents/Resources/PrivacyInfo.xcprivacy"
echo -e "  ${GREEN}.app signature, App Store entitlements, sandbox, and privacy manifest verified${NC}"

# --- Build .pkg ---
echo -e "\n${YELLOW}Building .pkg for App Store submission...${NC}"
PKG_PATH="src-tauri/target/release/bundle/macos/PhotoCurate.pkg"

productbuild --component "$APP_PATH" /Applications \
    --sign "$APPLE_INSTALLER_IDENTITY" \
    "$PKG_PATH" 2>&1
echo -e "  ${GREEN}.pkg created and signed${NC}"

pkgutil --check-signature "$PKG_PATH"

echo -e "\n${GREEN}=== Build Complete ===${NC}"
echo ""
echo "Artifacts:"
echo "  App bundle: $APP_PATH"
echo "  Package:    $PKG_PATH"
echo ""
echo "To upload to App Store Connect:"
echo "  export KEY_ID=YOUR_KEY_ID"
echo "  export ISSUER_ID=YOUR_ISSUER_ID"
echo "  xcrun altool --upload-app -f $PKG_PATH -t macOS --apiKey \$KEY_ID --apiIssuer \$ISSUER_ID"
