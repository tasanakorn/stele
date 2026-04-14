#!/usr/bin/env bash
set -euo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PREFIX="$HERE/../third_party/zenoh-c-prefix"
export PKG_CONFIG_PATH="$PREFIX/lib/pkgconfig:${PKG_CONFIG_PATH:-}"
export CGO_CFLAGS="-I$PREFIX/include"
export CGO_LDFLAGS="-L$PREFIX/lib -lzenohc -Wl,-rpath,$PREFIX/lib"
mkdir -p "$HERE/target"
cd "$HERE"
go build -o target/stylos ./cmd/stylos "$@"
