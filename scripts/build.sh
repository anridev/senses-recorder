#!/bin/bash
set -euo pipefail
source "$(dirname "$0")/env.sh"
cd "$SENSES_ROOT"
cargo build --release --locked
xcodebuild -project Senses.xcodeproj -scheme Senses -configuration Release \
  -derivedDataPath "$SENSES_ROOT/build/DerivedData" \
  CODE_SIGN_IDENTITY=- CODE_SIGN_STYLE=Manual build
mkdir -p build
ditto build/DerivedData/Build/Products/Release/Senses.app build/Senses.app
codesign --verify --deep --strict build/Senses.app
printf '\nBuilt %s/build/Senses.app\n' "$SENSES_ROOT"
