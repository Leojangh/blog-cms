#!/bin/bash
set -e

REMOTE_DIR="/mnt/scratch/overlay/system_ext/upper/blog-cms"
TARGET="aarch64-linux-android"
NDK="$HOME/Library/Android/sdk/ndk/28.0.13004108"
TOOLCHAIN="$NDK/toolchains/llvm/prebuilt/darwin-x86_64"
BIN="target/${TARGET}/release/blog-cms"

export CC_aarch64_linux_android="${TOOLCHAIN}/bin/aarch64-linux-android35-clang"
export AR_aarch64_linux_android="${TOOLCHAIN}/bin/llvm-ar"
export CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER="${TOOLCHAIN}/bin/aarch64-linux-android35-clang"

echo "==> 编译 Android (${TARGET})..."
cargo build --release --target $TARGET

echo "==> 推送二进制..."
adb push "$BIN" "${REMOTE_DIR}/blog-cms"
adb shell chmod 755 "${REMOTE_DIR}/blog-cms"

echo "==> 推送 .env（替换 DATABASE_PATH）..."
sed "s|DATABASE_PATH=.*|DATABASE_PATH=${REMOTE_DIR}/blog.db|" .env | adb shell "cat > ${REMOTE_DIR}/.env"

echo "==> 推送管理脚本..."
adb push blog-cms-android.sh "${REMOTE_DIR}/blog-cms.sh"
adb shell chmod 755 "${REMOTE_DIR}/blog-cms.sh"

echo "==> 部署完成"
echo "在设备上执行: ${REMOTE_DIR}/blog-cms.sh start"
