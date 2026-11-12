#!/bin/bash
set -euo pipefail
source "$(dirname "$0")/env.sh"
if ! command -v cargo >/dev/null 2>&1; then
  export CARGO_HOME="$SENSES_ROOT/.tools/cargo"
  export RUSTUP_HOME="$SENSES_ROOT/.tools/rustup"
  installer="$(mktemp -t senses-rustup)"
  trap 'rm -f "$installer"' EXIT
  curl --fail --location --proto '=https' --tlsv1.2 https://sh.rustup.rs -o "$installer"
  sh "$installer" -y --profile minimal --default-toolchain stable --no-modify-path
  export PATH="$CARGO_HOME/bin:$PATH"
fi
if command -v rustup >/dev/null 2>&1; then rustup component add clippy rustfmt; fi
CARGO_HTTP_MULTIPLEXING=false cargo fetch --manifest-path "$SENSES_ROOT/Cargo.toml" --locked
printf '\nReady. Run bash scripts/build.sh\n'
