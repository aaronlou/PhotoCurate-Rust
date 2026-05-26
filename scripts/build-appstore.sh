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

echo -e "\n${GREEN}=== Build Complete ===${NC}"
echo ""
echo "Artifacts:"
echo "  App bundle: src-tauri/target/release/bundle/macos/PhotoCurate.app"
echo ""
echo "Next steps:"
echo "  1. Test the signed app locally:"
echo "     open src-tauri/target/release/bundle/macos/PhotoCurate.app"
echo "  2. Upload to App Store Connect:"
echo "     xcrun altool --upload-app -f <path-to-pkg> -t macOS -u <apple-id> --apiKey <key-id> --apiIssuer <issuer-id>"
echo ""
echo "  Or use Transporter app:"
echo "     https://apps.apple.com/app/transporter/id1450874784"
