#!/usr/bin/env bash
# 生产环境构建脚本
# 构建前端 + Rust 后端，输出到 deploy/ 目录

set -euo pipefail
cd "$(dirname "$0")/.."

echo "==> 构建 Vue 前端..."
cd vue-frontend
npm ci --omit=dev
npm run build
echo "    前端构建完成: vue-frontend/dist/"

echo ""
echo "==> 构建 Rust 后端 (release)..."
cargo build --release
echo "    后端构建完成: target/release/qe_level_2_simpass_uuid_order"

echo ""
echo "============================================"
echo "  部署方式:"
echo ""
echo "  1. 复制 deploy/nginx.conf.example"
echo "     到 nginx 配置目录并修改"
echo ""
echo "  2. 启动 N 个 API 实例:"
echo "     export SERVER_PORT=8080"
echo "     ./target/release/qe_level_2_simpass_uuid_order &"
echo "     export SERVER_PORT=8081"
echo "     ./target/release/qe_level_2_simpass_uuid_order &"
echo ""
echo "  3. 前端静态文件由 nginx 直接服务"
echo "     root => vue-frontend/dist/"
echo ""
echo "============================================"
