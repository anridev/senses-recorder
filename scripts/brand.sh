#!/bin/bash
set -euo pipefail
SENSES_BRAND_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$SENSES_BRAND_ROOT"
mkdir -p build/SwiftModuleCache design
xcrun swiftc -module-cache-path build/SwiftModuleCache \
  native/SensesBrand.swift scripts/make-icon.swift -o build/make-icon
build/make-icon build/Senses.iconset design
iconutil -c icns build/Senses.iconset -o native/AppIcon.icns
cp build/Senses.iconset/icon_512x512@2x.png design/Senses-AppIcon.png
cp build/Senses.iconset/icon_128x128@2x.png design/Senses-AppIcon-256.png
