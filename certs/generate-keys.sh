#!/usr/bin/env bash
set -euo pipefail

CERT_DIR="$(cd "$(dirname "$0")" && pwd)"

# Generate RSA private key (2048-bit)
openssl genrsa -out "$CERT_DIR/private.pem" 2048

# Extract public key
openssl rsa -in "$CERT_DIR/private.pem" -pubout -out "$CERT_DIR/public.pem"

echo "Keys generated in $CERT_DIR"
echo "  Private: $CERT_DIR/private.pem"
echo "  Public:  $CERT_DIR/public.pem"
