#!/usr/bin/env bash
# 初始化 Android 工程并应用本项目的两处原生定制。
# 供 mobile.yml（正式构建）与 android-smoke.yml（冒烟验证）共用，
# 避免「验证的脚本」和「发布的脚本」逐渐漂移。
set -euo pipefail

echo "=== 初始化 Android 工程 ==="
npx tauri android init --ci

APP="src-tauri/gen/android/app/src/main"
RES="$APP/res"

echo "=== 1. MainActivity（状态栏避让）==="
MAIN_KT=$(find "$APP" -name "MainActivity.kt" | head -1)
if [ -z "$MAIN_KT" ]; then
  echo "::error::未找到 MainActivity.kt"
  exit 1
fi
PKG=$(grep -m1 "^package " "$MAIN_KT" | sed "s/^package //" | tr -d "\r")
echo "MainActivity: $MAIN_KT (package=$PKG)"
sed "s/{{PACKAGE}}/$PKG/" src-tauri/mobile/android/MainActivity.kt > "$MAIN_KT"
echo "--- 替换后的 MainActivity.kt ---"
cat "$MAIN_KT"

echo "=== 2. 启动器图标 ==="
# tauri android init 用的是官方模板图标（青黄双环），不会自动复制
# src-tauri/icons/android。必须显式覆盖，否则改图标只改了背景色。
for DPI in mdpi hdpi xhdpi xxhdpi xxxhdpi; do
  mkdir -p "$RES/mipmap-$DPI"
  cp -f "src-tauri/icons/android/mipmap-$DPI/"*.png "$RES/mipmap-$DPI/"
done
mkdir -p "$RES/mipmap-anydpi-v26" "$RES/values"
cp -f src-tauri/mobile/android/ic_launcher.xml "$RES/mipmap-anydpi-v26/ic_launcher.xml"
cp -f src-tauri/mobile/android/ic_launcher.xml "$RES/mipmap-anydpi-v26/ic_launcher_round.xml"
cp -f src-tauri/mobile/android/ic_launcher_background.xml "$RES/values/ic_launcher_background.xml"

# 模板自带的机器人矢量图优先级高于 mipmap，必须删掉
rm -f "$RES/drawable/ic_launcher_background.xml"
rm -f "$RES/drawable-v24/ic_launcher_foreground.xml"

echo "=== 3. 校验图标确实被覆盖 ==="
for DPI in mdpi hdpi xhdpi xxhdpi xxxhdpi; do
  if ! cmp -s "src-tauri/icons/android/mipmap-$DPI/ic_launcher.png" "$RES/mipmap-$DPI/ic_launcher.png"; then
    echo "::error::mipmap-$DPI/ic_launcher.png 覆盖失败"
    exit 1
  fi
  if ! cmp -s "src-tauri/icons/android/mipmap-$DPI/ic_launcher_foreground.png" "$RES/mipmap-$DPI/ic_launcher_foreground.png"; then
    echo "::error::mipmap-$DPI/ic_launcher_foreground.png 覆盖失败"
    exit 1
  fi
done
echo "✅ 图标已覆盖（5 密度 + 自适应图标 + 品牌蓝背景）"
find "$RES" -name "ic_launcher*" | sort
