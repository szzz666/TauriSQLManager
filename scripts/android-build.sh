#!/usr/bin/env bash
# 构建并签名 Android APK，产物放到 apk/ 目录。
# 供 mobile.yml 与 android-smoke.yml 共用。
set -euo pipefail

# 是否额外产出 AAB（上架 Google Play 用）。冒烟测试不需要，正式发布需要：
#   ANDROID_BUILD_AAB=1 bash scripts/android-build.sh
BUILD_ARGS=(--apk)
if [ "${ANDROID_BUILD_AAB:-0}" = "1" ]; then
  BUILD_ARGS+=(--aab)
  echo "=== 构建 release APK + AAB ==="
else
  echo "=== 构建 release APK ==="
fi
npx tauri android build "${BUILD_ARGS[@]}" --ci

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

# 调试密钥库：CI 上不预置，缺失时自己生成（调试证书，不可上架）
KS="$HOME/.android/debug.keystore"
if [ ! -f "$KS" ]; then
  echo "=== 生成调试密钥库 ==="
  mkdir -p "$(dirname "$KS")"
  keytool -genkeypair -v \
    -keystore "$KS" \
    -storepass android -alias androiddebugkey -keypass android \
    -keyalg RSA -keysize 2048 -validity 10000 \
    -dname "CN=Android Debug,O=Android,C=US"
fi
echo "调试密钥库: $KS"

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
