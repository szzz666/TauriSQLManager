#!/usr/bin/env bash
# 构建并签名 Android APK，产物放到 apk/ 目录。
# 供 mobile.yml 与 android-smoke.yml 共用。
set -euo pipefail

echo "=== 构建 release APK ==="
npx tauri android build --apk --ci

OUT="src-tauri/gen/android/app/build/outputs"
APK=$(find "$OUT" -type f -name "*.apk" -path "*/release/*" | head -1 || true)
if [ -z "$APK" ]; then
  echo "::error::未找到 release APK"
  find "$OUT" -name "*.apk" 2>/dev/null | head -10 || true
  exit 1
fi
echo "待签名 APK: $APK"

APKSIGNER=$(find "${ANDROID_HOME:-/usr/local/lib/android/sdk}/build-tools" -name apksigner -type f 2>/dev/null | sort -V | tail -1)
if [ -z "$APKSIGNER" ]; then
  echo "::error::未找到 apksigner"
  exit 1
fi

KS="$HOME/.android/debug.keystore"
if [ ! -f "$KS" ]; then
  echo "::error::缺少调试密钥库 $KS"
  exit 1
fi

"$APKSIGNER" sign \
  --ks "$KS" \
  --ks-pass pass:android \
  --key-pass pass:android \
  --ks-key-alias androiddebugkey \
  --v1-signing-enabled true \
  --v2-signing-enabled true \
  --v3-signing-enabled true \
  "$APK"

"$APKSIGNER" verify --verbose "$APK" | head -12

mkdir -p apk
cp -v "$APK" apk/app-installable.apk
ls -lh apk/
