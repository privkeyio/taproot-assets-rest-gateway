#!/bin/bash

# Script to help find Taproot Assets and LND macaroon files

echo "🔍 Searching for Taproot Assets and LND macaroon files..."
echo ""

# Search in common locations
SEARCH_PATHS=(
    "$HOME/.tapd"
    "$HOME/.lnd"
    "$HOME/.polar"
    "$HOME/polar"
    "/home/*/.tapd"
    "/home/*/.lnd"
    "/home/*/.polar"
)

echo "📁 Searching for Taproot Assets macaroons:"
for path in "${SEARCH_PATHS[@]}"; do
    if [[ -d "$path" ]]; then
        find "$path" -name "admin.macaroon" -path "*/tapd/*" 2>/dev/null | while read -r file; do
            echo "  Found: $file"
        done
    fi
done

echo ""
echo "📁 Searching for LND macaroons:"
for path in "${SEARCH_PATHS[@]}"; do
    if [[ -d "$path" ]]; then
        find "$path" -name "admin.macaroon" -path "*/lnd/*" 2>/dev/null | while read -r file; do
            echo "  Found: $file"
        done
    fi
done

echo ""
echo "💡 Tips:"
echo "  - Copy the paths above to your .env.local file"
echo "  - Use TAPD_MACAROON_PATH for Taproot Assets macaroons"
echo "  - Use LND_MACAROON_PATH for LND macaroons"
echo "  - For Polar development, set TLS_VERIFY=false"
echo ""
echo "📝 Example .env.local entry:"
echo "  TAPD_MACAROON_PATH=/path/to/tapd/admin.macaroon"
echo "  LND_MACAROON_PATH=/path/to/lnd/admin.macaroon"
