#!/usr/bin/env bash

set -e

export RUSTFLAGS='-D warnings'
export RUSTDOCFLAGS='-D warnings'

check() {
  echo "check feature \`$1\`"
  cargo doc -q
  cargo fmt --check --all
  cargo build --features "$1"
  cargo build --features "$1" --target "x86_64-unknown-linux-musl"
  cargo clippy --features "$1"
  cargo test --features "$1" -q
  echo ''
}

check serde
check latest
check legacy
check linux-6.13
check linux-6.11
check linux-6.8
check linux-6.6
check linux-6.3
check linux-6.1
check linux-6.0
check linux-5.18
check linux-5.17
check linux-5.16
check linux-5.13
check linux-5.12
check linux-5.11
check linux-5.9
check linux-5.7
check linux-5.5
check linux-5.4
check linux-5.1
check linux-4.17
check linux-4.15
check linux-4.14
check linux-4.12
check linux-4.10
check linux-4.8
check linux-4.7
check linux-4.5
check linux-4.4
check linux-4.3
check linux-4.2
check linux-4.1
check linux-4.0
