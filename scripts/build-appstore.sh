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
check_var APPLE_PROVIDER_SHORT_NAME

# --- Verify certificate exists in Keychain ---
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
npm run build

# --- Build the Tauri app with App Store signing ---
echo -e "\n${YELLOW}Building .app bundle (Mac App Store signed)...${NC}"

export APPLE_SIGNING_IDENTITY
export APPLE_PROVIDER_SHORT_NAME

npm run tauri-build -- --bundles app

APP_PATH="src-tauri/target/release/bundle/macos/PhotoCurate.app"
PROFILE_PATH="$HOME/Library/MobileDevice/Provisioning Profiles/com.photocurate.provisionprofile"

# --- Embed provisioning profile ---
echo -e "\n${YELLOW}Embedding provisioning profile...${NC}"
if [ -f "$PROFILE_PATH" ]; then
    cp "$PROFILE_PATH" "$APP_PATH/Contents/embedded.provisionprofile"
    echo -e "  ${GREEN}Provisioning profile embedded${NC}"
else
    echo -e "  ${RED}Provisioning profile not found at $PROFILE_PATH${NC}"
    exit 1
fi

# --- Re-sign after embedding profile ---
echo -e "\n${YELLOW}Re-signing with embedded provisioning profile...${NC}"
codesign --sign "$APPLE_SIGNING_IDENTITY" \
    --entitlements src-tauri/Entitlements.plist \
    --options runtime \
    --deep \
    --force \
    "$APP_PATH" 2>&1
echo -e "  ${GREEN}Re-signed${NC}"

# --- Build .pkg ---
echo -e "\n${YELLOW}Building .pkg for App Store submission...${NC}"
INSTALLER_IDENTITY="3rd Party Mac Developer Installer: Aaron Lou (V63B559WYX)"
PKG_PATH="src-tauri/target/release/bundle/macos/PhotoCurate.pkg"

productbuild --component "$APP_PATH" /Applications \
    --sign "$INSTALLER_IDENTITY" \
    "$PKG_PATH" 2>&1
echo -e "  ${GREEN}.pkg created and signed${NC}"

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
