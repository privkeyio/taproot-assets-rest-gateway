#!/bin/bash

# Taproot Assets REST Gateway Setup Script
set -e

echo "🚀 Setting up Taproot Assets REST Gateway..."

# Check if Docker is available
if command -v docker &> /dev/null; then
    HAS_DOCKER=true
    echo "✅ Docker found"
else
    HAS_DOCKER=false
    echo "⚠️  Docker not found - will set up for source build only"
fi

# Check if Rust is available
if command -v cargo &> /dev/null; then
    HAS_RUST=true
    echo "✅ Rust found"
else
    HAS_RUST=false
    echo "⚠️  Rust not found - will set up for Docker only"
fi

# Copy environment file
if [ ! -f .env ]; then
    echo "📝 Creating .env file..."
    cp .env.example .env
    echo "✅ Created .env file - please edit it with your settings"
else
    echo "📝 .env file already exists"
fi

# Detect common paths
echo "🔍 Looking for common macaroon locations..."

# Check for Polar
POLAR_BASE="$HOME/.polar/networks"
if [ -d "$POLAR_BASE" ]; then
    echo "📁 Found Polar installation at $POLAR_BASE"
    echo "   Example paths:"
    find "$POLAR_BASE" -name "admin.macaroon" -type f 2>/dev/null | head -5 | while read -r path; do
        echo "   - $path"
    done
fi

# Check for standard LND
LND_BASE="$HOME/.lnd"
if [ -d "$LND_BASE" ]; then
    echo "📁 Found LND installation at $LND_BASE"
    find "$LND_BASE" -name "admin.macaroon" -type f 2>/dev/null | head -3 | while read -r path; do
        echo "   - $path"
    done
fi

# Check for standard tapd
TAPD_BASE="$HOME/.tapd"
if [ -d "$TAPD_BASE" ]; then
    echo "📁 Found Tapd installation at $TAPD_BASE"
    find "$TAPD_BASE" -name "admin.macaroon" -type f 2>/dev/null | head -3 | while read -r path; do
        echo "   - $path"
    done
fi

echo ""
echo "📋 Next Steps:"
echo ""
echo "1. Edit .env file with your specific paths:"
echo "   nano .env"
echo ""

if [ "$HAS_RUST" = true ]; then
    echo "2a. Run from source:"
    echo "    cargo run --release"
    echo ""
fi

if [ "$HAS_DOCKER" = true ]; then
    echo "2b. Run with Docker (development):"
    echo "    docker build -t taproot-assets-rest-gateway ."
    echo "    docker-compose up -d"
    echo ""
    echo "2c. Run with Docker (production):"
    echo "    docker build -t taproot-assets-rest-gateway ."
    echo "    docker-compose -f docker-compose-bridge.yml up -d"
    echo ""
fi

echo "3. Test the connection:"
echo "   curl http://localhost:8080/health"
echo ""
echo "4. Check your configuration:"
echo "   curl http://localhost:8080/v1/taproot-assets/getinfo"
echo ""

echo "🎉 Setup complete! Remember to:"
echo "   - Update the macaroon paths in .env"
echo "   - Set TLS_VERIFY=true for production"
echo "   - Configure CORS_ORIGINS for your frontend"
echo ""
echo "For help: https://github.com/privkeyio/taproot-assets-rest-gateway/issues"
