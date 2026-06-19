#!/bin/bash
# 自动压测脚本 — 容器内执行
# 启动 N 个 API 实例 → 跑 hey 压测 → 输出结果
set -euo pipefail

INSTANCES=${INSTANCES:-3}
BASE_PORT=${BASE_PORT:-8080}
CONCURRENCY=${CONCURRENCY:-100}
REQUESTS=${REQUESTS:-5000}

# 工具检查
for cmd in app hey curl; do
    if ! command -v $cmd &>/dev/null; then
        echo "❌ 找不到 $cmd"
        exit 1
    fi
done

echo "============================================"
echo "  自动化压测开始"
echo "============================================"
echo "  实例数:    ${INSTANCES}"
echo "  并发:      ${CONCURRENCY}"
echo "  请求数:    ${REQUESTS}"
echo "  DB:        ${PG_HOST:-localhost}:${PG_PORT:-5432}/${PG_DBNAME:-?}"
echo "  Redis:     ${REDIS_URL:-redis://localhost:6379}"
echo "============================================"

# ── 1. 启动 N 个 API 实例 ──────────────────────────────────
echo ""
echo ">>> 启动 ${INSTANCES} 个实例..."

for i in $(seq 0 $((INSTANCES - 1))); do
    PORT=$((BASE_PORT + i))
    SERVER_PORT=$PORT app &>/tmp/server-${PORT}.log &
    echo "  实例 $((i+1)) → 0.0.0.0:${PORT}  PID=$!"
done

# ── 2. 等待就绪 ────────────────────────────────────────────
echo ""
echo -n "等待实例就绪"
for i in $(seq 0 $((INSTANCES - 1))); do
    PORT=$((BASE_PORT + i))
    for retry in $(seq 1 20); do
        if curl -sf http://127.0.0.1:${PORT}/health >/dev/null 2>&1; then
            echo -n " ✅:${PORT}"
            break
        fi
        sleep 1
    done
done
echo ""

# ── 3. 单实例基准 (仅测端口 8080) ─────────────────────────
echo ""
echo "============================================"
echo "  📊 阶段 A: 单实例基准测试 (端口 ${BASE_PORT})"
echo "============================================"

bench_one() {
    local label="$1" url="$2" c="${3:-$CONCURRENCY}" n="${4:-$REQUESTS}"
    echo ""
    echo "--- ${label} ---"
    hey -n "$n" -c "$c" -t 30 "$url" 2>&1 | grep -E "Requests/sec|Total:|Slowest|Fastest|Average|\[2|\[3|\[4|\[5"
}

bench_one "GET /health (纯路由)" "http://127.0.0.1:${BASE_PORT}/health"
bench_one "GET /api/qps/current (读 Redis)" "http://127.0.0.1:${BASE_PORT}/api/qps/current?api_path=/api"
bench_one "GET /api/system/configs (读 PG)" "http://127.0.0.1:${BASE_PORT}/api/system/configs"
bench_one "GET /api/developers (列举 PG)" "http://127.0.0.1:${BASE_PORT}/api/developers?page=1\&page_size=10"

# ── 4. 多实例并发压测 ─────────────────────────────────────
echo ""
echo "============================================"
echo "  📊 阶段 B: ${INSTANCES} 实例并发压测"
echo "============================================"

# 同时向所有实例发压，看总吞吐
for i in $(seq 0 $((INSTANCES - 1))); do
    PORT=$((BASE_PORT + i))
    echo ""
    echo "--- 实例 $((i+1)) :${PORT} /health (并发 $((CONCURRENCY / INSTANCES))) ---"
    hey -z 10s -c $((CONCURRENCY / INSTANCES)) "http://127.0.0.1:${PORT}/health" 2>&1 | grep -E "Requests/sec|Total:|Average|\[2|\[3|\[4"
done

# ── 5. 混合扣量场景 (写密集型) ────────────────────────────
echo ""
echo "============================================"
echo "  📊 阶段 C: 扣量场景压测 (PG + Redis 写)"
echo "============================================"

# 准备压测用的开发者
DEV_UUID="550e8400-e29b-41d4-a716-446655440000"
curl -sf -X POST "http://127.0.0.1:${BASE_PORT}/api/developers" \
  -H "Content-Type: application/json" \
  -d "{\"developer_uuid\":\"${DEV_UUID}\",\"developer_name\":\"bench\",\"deduction_limit\":999999,\"deduction_available\":999999,\"recovery_interval_secs\":60,\"recovery_amount\":100}" >/dev/null 2>&1 || true

echo ""
echo "--- POST /api/deductions/initiate (创建) ---"
PAYLOAD="{\"developer_uuid\":\"${DEV_UUID}\",\"amount\":1}"
echo "$PAYLOAD" > /tmp/bench-payload.json
hey -n 5000 -c 50 -m POST -D /tmp/bench-payload.json \
  -H "Content-Type: application/json" \
  -t 30 \
  "http://127.0.0.1:${BASE_PORT}/api/deductions/initiate" 2>&1 | grep -E "Requests/sec|Total:|Average|\[2|\[3|\[4|\[5"

# ── 6. 聚合结果: hey 直接测3个端口的总QPS ────────────────
echo ""
echo "============================================"
echo "  📊 阶段 D: 3 实例聚合吞吐 (同时压 3 端口)"
echo "============================================"

# 多线程：同时向3个端口发压
for port in $(seq $BASE_PORT $((BASE_PORT + INSTANCES - 1))); do
    (
        r=$(hey -z 10s -c $((CONCURRENCY / INSTANCES)) "http://127.0.0.1:${port}/health" 2>&1 | grep "Requests/sec" | awk '{print $2}')
        echo "  端口 ${port}: ${r} QPS"
    ) &
done
wait

# ── 7. 系统资源 ────────────────────────────────────────────
echo ""
echo "============================================"
echo "  📊 资源使用"
echo "============================================"
echo ""
echo "--- CPU (top 5 app 进程) ---"
ps aux --sort=-%cpu 2>/dev/null | grep app | head -5 || ps aux 2>/dev/null | head -5
echo ""
echo "--- 内存 ---"
free -h 2>/dev/null | head -2 || echo "(不可用)"
echo ""
echo "--- 连接数 ---"
ss -tn | grep -c "808" 2>/dev/null && echo "个 TCP 连接到 API 端口" || true

# ── 8. 完成 ────────────────────────────────────────────────
echo ""
echo "============================================"
echo "  ✅ 压测完成"
echo "============================================"
echo "  系统: Debian $(cat /etc/debian_version 2>/dev/null || echo '?')"
echo "  CPU: $(nproc) 核"
echo "  内存: $(free -h 2>/dev/null | awk '/Mem/{print $2}')"
echo "  实例: ${INSTANCES}"
echo "  结果见上方各阶段"
echo "============================================"

# 保持容器存活，方便查看日志
sleep 30
