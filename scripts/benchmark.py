#!/usr/bin/env python3
"""轻量级 HTTP 压测 — 只依赖 Python 标准库"""
import time
import sys
import statistics
import urllib.request
from concurrent.futures import ThreadPoolExecutor, as_completed

def bench(url: str, concurrency: int, total: int, name: str) -> float:
    def req():
        t0 = time.perf_counter()
        try:
            r = urllib.request.urlopen(url, timeout=30)
            r.read()  # 读完响应体
            t1 = time.perf_counter()
            return (t1 - t0) * 1000
        except Exception as e:
            return None

    t0 = time.perf_counter()
    ok, latencies = 0, []
    with ThreadPoolExecutor(max_workers=concurrency) as pool:
        futs = [pool.submit(req) for _ in range(total)]
        for f in as_completed(futs):
            r = f.result()
            if r is not None:
                ok += 1
                latencies.append(r)
    elapsed = time.perf_counter() - t0
    qps = ok / elapsed if elapsed > 0 else 0
    avg = statistics.mean(latencies) if latencies else 0
    latencies.sort()
    p50 = statistics.median(latencies) if latencies else 0
    p99 = latencies[int(len(latencies) * 0.99)] if latencies else 0

    print(f"  {name:40s}  QPS={qps:>7.0f}  avg={avg:>6.1f}ms  P50={p50:>6.1f}ms  P99={p99:>6.1f}ms  ok={ok}/{total}")
    return qps

if __name__ == "__main__":
    PORT = sys.argv[1] if len(sys.argv) > 1 else "8080"
    C = 50   # 并发
    N = 2000 # 每端点请求数
    BASE = f"http://127.0.0.1:{PORT}"

    print(f"\n{'='*65}")
    print(f"  压测配置: 并发={C}, 每端点请求数={N}, 端口={PORT}")
    print(f"  预计耗时: ~{(N / (C * 50)) * N / 10:.0f}s")
    print(f"{'='*65}\n")

    results = []
    results.append(bench(f"{BASE}/health", C, N, "GET /health (纯路由)"))
    results.append(bench(f"{BASE}/api/qps/current?api_path=/api", C, N, "GET /api/qps/current (读 Redis)"))
    results.append(bench(f"{BASE}/api/system/configs", C, N, "GET /api/system/configs (读 PG)"))
    results.append(bench(f"{BASE}/api/developers?page=1&page_size=10", C, N, "GET /api/developers (列举 PG)"))

    print(f"\n{'='*65}")
    print(f"  平均 QPS: {statistics.mean(results):.0f} (所有端点)")
    print(f"  瓶颈分析: 见上方最低 QPS 端点")
    print(f"{'='*65}\n")
