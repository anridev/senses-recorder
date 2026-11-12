#!/bin/bash
set -euo pipefail
source "$(dirname "$0")/env.sh"
cd "$SENSES_ROOT"
mkdir -p build
export SENSES_SOAK_SECONDS="${SENSES_SOAK_SECONDS:-14400}"
export SENSES_SOAK_REPORT="${SENSES_SOAK_REPORT:-$SENSES_ROOT/build/soak-report.json}"
if [ "${1:-}" = "--fast" ]; then export SENSES_SOAK_FAST=1; fi
cargo test --release --locked soak::four_hour_recording -- --ignored --nocapture --test-threads=1
