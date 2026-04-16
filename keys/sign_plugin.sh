#!/usr/bin/env bash
set -euo pipefail

WASM="$1"
PRIVKEY_B64="$2"

SHA256=$(sha256sum "$WASM" | awk '{print $1}')
HASH_BIN=$(echo "$SHA256" | xxd -r -p)
SIG_B64=$(echo "$HASH_BIN" | openssl pkeyutl -sign -inkey <(echo "$PRIVKEY_B64" | base64 -d) -pkeyopt digest:none 2>/dev/null | base64 -w0)

echo "sha256    = \"$SHA256\""
echo "signature = \"$SIG_B64\""
