#!/usr/bin/env bash
# Android 冒烟测试：安装 APK、启动、抓 logcat 与截图。
#
# 注意：reactivecircus/android-emulator-runner 是「逐行」用 sh -c 执行 script 的，
# 变量不会跨行保留。所以 workflow 里只写一行 `bash scripts/android-smoke.sh`，
# 真正逻辑放在本文件。
set -uo pipefail

PKG=top.szzz666.taurisqlmanager
FAIL=0

APK=$(ls apk/*.apk 2>/dev/null | head -1)
if [ -z "$APK" ]; then
  echo "::error::没有找到 apk/*.apk"
  ls -la
  exit 1
fi
echo "安装: $APK"

if ! adb install -r -t "$APK"; then
  echo "::error::adb install 失败"
  exit 1
fi
adb shell pm list packages | grep -i szzz || true

adb logcat -c
echo "--- 启动 ---"
adb shell am start -W -n "$PKG/.MainActivity"
sleep 25

PID=$(adb shell pidof "$PKG" | tr -d "\r")
echo "PID=[$PID]"

echo "--- 当前前台 Activity ---"
adb shell dumpsys activity activities | grep -E "mResumedActivity|topResumedActivity" | head -3

adb logcat -d > logcat.txt
wc -l logcat.txt

echo "===== FATAL EXCEPTION ====="
grep -n -A 45 "FATAL EXCEPTION" logcat.txt | head -150
echo "===== 崩溃线索 ====="
grep -n -E "AndroidRuntime|NoClassDefFound|NoSuchMethod|ClassNotFound|UnsatisfiedLink|Unable to instantiate|ANR in|Force finishing|ResourcesNotFound" logcat.txt | head -60
echo "===== 应用日志 ====="
grep -iE "$PKG|tauri|RustWebView|chromium|WebViewFactory" logcat.txt | tail -80

echo "--- 截图 ---"
adb exec-out screencap -p > screen-main.png || true
ls -l screen-main.png 2>/dev/null

# 再截一张去掉状态栏内容的对照图（看顶部是否压字）
adb shell settings put global policy_control immersive.status=* 2>/dev/null || true
sleep 2
adb exec-out screencap -p > screen-immersive.png || true
adb shell settings put global policy_control null 2>/dev/null || true

if [ -z "$PID" ]; then
  echo "::error::应用进程已退出（启动即崩溃），见上方 FATAL EXCEPTION"
  FAIL=1
fi
if grep -q "FATAL EXCEPTION" logcat.txt; then
  echo "::error::logcat 中出现 FATAL EXCEPTION"
  FAIL=1
fi

if [ $FAIL -eq 0 ]; then
  echo "✅ 应用启动正常，进程存活: $PID"
fi
exit $FAIL
