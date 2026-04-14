#!/usr/bin/env bash
set -euo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
CERTS="$HERE/certs"
mkdir -p "$CERTS"
cd "$CERTS"

CA_KEY="stylos-dev-ca.key"
CA_CRT="stylos-dev-ca.crt"
SRV_KEY="stylos-dev.key"
SRV_CSR="stylos-dev.csr"
SRV_CRT="stylos-dev.crt"
SRV_EXT="stylos-dev.ext"
DAYS=3650

openssl genrsa -out "$CA_KEY" 4096
openssl req -x509 -new -nodes -key "$CA_KEY" -sha256 -days "$DAYS" \
  -subj "/CN=Stylos Dev CA" -out "$CA_CRT"

openssl genrsa -out "$SRV_KEY" 4096
openssl req -new -key "$SRV_KEY" -subj "/CN=stylos-dev" -out "$SRV_CSR"

cat > "$SRV_EXT" <<EOF
authorityKeyIdentifier=keyid,issuer
basicConstraints=CA:FALSE
keyUsage = digitalSignature, keyEncipherment
extendedKeyUsage = serverAuth, clientAuth
subjectAltName = @alt_names
[alt_names]
DNS.1 = localhost
DNS.2 = stylos-dev
IP.1  = 127.0.0.1
IP.2  = 0.0.0.0
EOF

openssl x509 -req -in "$SRV_CSR" -CA "$CA_CRT" -CAkey "$CA_KEY" -CAcreateserial \
  -out "$SRV_CRT" -days "$DAYS" -sha256 -extfile "$SRV_EXT"

rm -f "$SRV_CSR" "$SRV_EXT" stylos-dev-ca.srl
echo "Generated:"
ls -1 "$CERTS"
