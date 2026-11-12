#!/bin/bash
set -euo pipefail
source "$(dirname "$0")/env.sh"
cd "$SENSES_ROOT"
cargo test --locked -- --test-threads=1
cargo clippy --all-targets --locked -- -D warnings
mkdir -p build/SwiftModuleCache
xcrun swiftc -module-cache-path "$SENSES_ROOT/build/SwiftModuleCache" \
  native/SourceMembership.swift tests/SourceMembershipTests.swift -o build/source-membership-tests
build/source-membership-tests
