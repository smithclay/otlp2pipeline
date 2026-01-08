#!/bin/bash
# scripts/check-size.sh
set -e

echo "Building release WASM..."
cargo build --release --target wasm32-unknown-unknown

WASM_FILE="target/wasm32-unknown-unknown/release/otlp2pipeline.wasm"

if [ ! -f "$WASM_FILE" ]; then
    echo "ERROR: WASM file not found"
    exit 1
fi

RAW_SIZE=$(ls -l "$WASM_FILE" | awk '{print $5}')
RAW_SIZE_MB=$(echo "scale=2; $RAW_SIZE / 1048576" | bc)

COMPRESSED_SIZE=$(gzip -c "$WASM_FILE" | wc -c)
COMPRESSED_SIZE_MB=$(echo "scale=2; $COMPRESSED_SIZE / 1048576" | bc)

echo ""
echo "=== Bundle Size Report ==="
echo "Raw WASM:        ${RAW_SIZE_MB} MB"
echo "Compressed:      ${COMPRESSED_SIZE_MB} MB"
echo ""

# Check against limits
if (( $(echo "$COMPRESSED_SIZE_MB > 3" | bc -l) )); then
    echo "WARNING: Exceeds free tier limit (3 MB compressed)"
fi

if (( $(echo "$COMPRESSED_SIZE_MB > 10" | bc -l) )); then
    echo "ERROR: Exceeds paid tier limit (10 MB compressed)"
    exit 1
fi

echo "OK: Within Cloudflare Workers limits"
