#!/bin/bash
set -e

SERVER="${DEPLOY_SERVER:?请设置 DEPLOY_SERVER 环境变量，例如 export DEPLOY_SERVER=root@your-ip}"
REMOTE_DIR="/opt/blog-cms"
TARGET="x86_64-unknown-linux-musl"
BIN="target/${TARGET}/release/blog-cms"

echo "==> 编译后端..."
cargo zigbuild --release --target $TARGET

echo "==> 停止远程服务..."
ssh $SERVER "systemctl stop blog-cms"

echo "==> 上传后端..."
scp $BIN $SERVER:$REMOTE_DIR/blog-cms

echo "==> 启动服务..."
ssh $SERVER "systemctl start blog-cms"

echo "==> 后端部署完成"
ssh $SERVER "systemctl status blog-cms --no-pager | head -5"

echo ""
echo "==> 构建前端..."
cd frontend
PUBLIC_API_URL=https://api.genlz.com bun run build
echo "==> 前端构建完成，请将 frontend/dist/ 上传到 Cloudflare Pages (Direct Upload)"
