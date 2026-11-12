#!/bin/bash
if [ -n "${ZSH_VERSION:-}" ]; then
  SENSES_ENV_FILE="${(%):-%x}"
else
  SENSES_ENV_FILE="${BASH_SOURCE[0]}"
fi
SENSES_ROOT="$(cd "$(dirname "$SENSES_ENV_FILE")/.." && pwd)"
if [ -x "$SENSES_ROOT/.tools/cargo/bin/cargo" ]; then
  export CARGO_HOME="$SENSES_ROOT/.tools/cargo"
  export RUSTUP_HOME="$SENSES_ROOT/.tools/rustup"
  export PATH="$CARGO_HOME/bin:$PATH"
fi
export MACOSX_DEPLOYMENT_TARGET=26.0
