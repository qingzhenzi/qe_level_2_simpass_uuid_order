use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use serde_json::Value;
use uuid::Uuid;

#[tokio::main]
async fn main() {
    let args: Vec<std::ffi::OsString> = std::env::args_os().collect();
    let base_url = args
        .get(1)
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "http://localhost:8080".to_string());

    let get_concurrency = vec![500u32, 800, 1000];
    let write_concurrency = vec![100u32, 200, 300, 500];
    let phase_secs = 8u64;

    println!();
    println!("╔══════════════════════════════════════════════════════════════════════╗");
    println!("║              STRESS TEST — UUID Deduction System                    ║");
    println!("║              Target: {}              ║", base_url);
    println!("╚══════════════════════════════════════════════════════════════════════╝");
    println!();

    // =====================================================================
    // Phase 0: Setup — 预创建一批测试开发者（带足够 risky_marks）
    // =====================================================================
    println!("━━━ [Setup] 预创建测试开发者 ━━━");
    let dev_count = 50u32;
    let dev_uuids = create_test_developers(&base_url, dev_count).await;
    let dev_pool = Arc::new(dev_uuids);
    println!();

    // =====================================================================
    // Phase 1: 只读 GET 接口压测
    // =====================================================================
    println!("━━━ [Phase 1] 只读 GET 接口 ━━━");
    let get_endpoints: Vec<(&str, String)> = vec![
        ("GET  /health", format!("{}/health", base_url)),
        (
            "GET  /api/developers?page=1&page_size=20",
            format!("{}/api/developers?page=1&page_size=20", base_url),
        ),
        (
            "GET  /api/deductions/transactions?page=1&page_size=20",
            format!(
                "{}/api/deductions/transactions?page=1&page_size=20",
                base_url
            ),
        ),
    ];

    for (label, url) in &get_endpoints {
        run_get_phase(label, url, &get_concurrency, phase_secs).await;
    }

    // =====================================================================
    // Phase 2: 扣款两阶段流程压测 (Initiate → Confirm)
    // =====================================================================
    println!("━━━ [Phase 2] 扣款两阶段流程 (Initiate → Confirm) ━━━");
    run_deduction_flow_phase(&base_url, &dev_pool, &write_concurrency, phase_secs).await;

    // =====================================================================
    // Phase 3: 开发者 CRUD 流程压测 (Create → Get → Update → Delete)
    // =====================================================================
    println!("━━━ [Phase 3] 开发者 CRUD (Create → Get → Update → Delete) ━━━");
    run_developer_crud_phase(&base_url, &write_concurrency, phase_secs).await;

    println!();
    println!("━━━ 全部压力测试完成 ━━━");
}

// ============================================================================
// 辅助函数
// ============================================================================

fn print_header() {
    println!(
        "  {:>6}  {:>8}  {:>8}  {:>8}  {:>8}  {:>8}  {:>8}  {:>8}",
        "并发数", "请求总量", "成功", "失败", "QPS", "p50(ms)", "p90(ms)", "p99(ms)"
    );
    println!("  {}", "─".repeat(78));
}

fn print_row(elapsed_secs: f64, concurrency: u32, r: &PhaseResult) {
    let qps = (r.total as f64) / elapsed_secs;
    let p50 = percentile(&r.latencies, 50.0);
    let p90 = percentile(&r.latencies, 90.0);
    let p99 = percentile(&r.latencies, 99.0);

    println!(
        "  {:>6}  {:>8}  {:>8}  {:>8}  {:>8.0}  {:>8.1}  {:>8.1}  {:>8.1}",
        concurrency, r.total, r.success, r.fail, qps, p50, p90, p99,
    );
}

fn percentile(sorted: &[f64], pct: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    let idx = ((pct / 100.0) * sorted.len() as f64).ceil() as usize;
    let idx = idx.min(sorted.len() - 1);
    sorted[idx]
}

#[derive(Default)]
struct PhaseResult {
    total: u64,
    success: u64,
    fail: u64,
    latencies: Vec<f64>,
}

// ============================================================================
// 公共压力测试框架
// ============================================================================

/// 创建一个共享的 reqwest Client
fn build_client() -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .unwrap()
}

// ============================================================================
// Phase 0: 预创建测试开发者
// ============================================================================

async fn create_test_developers(base_url: &str, count: u32) -> Vec<Uuid> {
    let client = build_client();
    let mut uuids = Vec::with_capacity(count as usize);

    for i in 0..count {
        let dev_uuid = Uuid::new_v4();
        let body = serde_json::json!({
            "developer_uuid": dev_uuid,
            "developer_name": format!("stress-test-dev-{}", i),
            "successful_auths": 0,
            "risky_marks_available": 1_000_000,
            "total_risky_marks_earned": 1_000_000,
            "total_risky_marks_used": 0,
            "auths_needed_for_next_mark": 1000,
        });

        let url = format!("{}/api/developers", base_url);
        match client
            .post(&url)
            .json(&body)
            .send()
            .await
        {
            Ok(resp) if resp.status().is_success() => {
                uuids.push(dev_uuid);
                print!(".");
            }
            Ok(resp) => {
                let status = resp.status();
                let text = resp.text().await.unwrap_or_default();
                eprintln!(
                    "\n[WARN] 创建开发者失败 ({}): status={}, body={}",
                    dev_uuid, status, text
                );
            }
            Err(e) => {
                eprintln!("\n[WARN] 创建开发者请求失败 ({}): {}", dev_uuid, e);
            }
        }

        // 稍微错开创建时间，避免瞬间高负载
        if i % 10 == 9 {
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    }

    println!("  已创建 {} 个测试开发者", uuids.len());
    uuids
}

// ============================================================================
// Phase 1: 只读 GET 接口
// ============================================================================

async fn run_get_phase(label: &str, url: &str, concurrency_levels: &[u32], phase_secs: u64) {
    println!("  ▸ {}", label);
    print_header();
    let duration = Duration::from_secs(phase_secs);
    for &concurrency in concurrency_levels {
        let result = run_get_scenario(url, concurrency, duration).await;
        print_row(phase_secs as f64, concurrency, &result);
    }
    println!();
}

async fn run_get_scenario(url: &str, concurrency: u32, duration: Duration) -> PhaseResult {
    let stop = Arc::new(AtomicBool::new(false));
    let total = Arc::new(AtomicU64::new(0));
    let success = Arc::new(AtomicU64::new(0));
    let fail = Arc::new(AtomicU64::new(0));
    let latencies = Arc::new(parking_lot::Mutex::new(Vec::new()));
    let client = build_client();

    let mut handles = Vec::new();
    for _ in 0..concurrency {
        let stop = stop.clone();
        let total = total.clone();
        let success = success.clone();
        let fail = fail.clone();
        let latencies = latencies.clone();
        let client = client.clone();
        let url = url.to_string();

        handles.push(tokio::spawn(async move {
            loop {
                if stop.load(Ordering::Relaxed) {
                    return;
                }
                let start = Instant::now();
                let resp = client.get(&url).send().await;
                let elapsed = start.elapsed().as_secs_f64() * 1000.0;
                total.fetch_add(1, Ordering::Relaxed);
                match resp {
                    Ok(r) if r.status().is_success() => {
                        success.fetch_add(1, Ordering::Relaxed);
                        latencies.lock().push(elapsed);
                    }
                    _ => {
                        fail.fetch_add(1, Ordering::Relaxed);
                    }
                }
            }
        }));
    }

    tokio::time::sleep(duration).await;
    stop.store(true, Ordering::Relaxed);

    for h in handles {
        let _ = h.await;
    }

    let t = total.load(Ordering::Relaxed);
    let s = success.load(Ordering::Relaxed);
    let f = fail.load(Ordering::Relaxed);
    let mut lat = latencies.lock().clone();
    lat.sort_by(|a, b| a.partial_cmp(b).unwrap());

    PhaseResult {
        total: t,
        success: s,
        fail: f,
        latencies: lat,
    }
}

// ============================================================================
// Phase 2: 扣款两阶段流程 (Initiate → Confirm)
// 每个并发任务循环执行: POST initiate -> POST confirm
// ============================================================================

async fn run_deduction_flow_phase(
    base_url: &str,
    dev_pool: &Arc<Vec<Uuid>>,
    concurrency_levels: &[u32],
    phase_secs: u64,
) {
    println!("  ▸ POST /api/deductions/initiate → POST /api/deductions/confirm");
    print_header();
    let duration = Duration::from_secs(phase_secs);
    for &concurrency in concurrency_levels {
        let result =
            run_deduction_flow_scenario(base_url, dev_pool, concurrency, duration).await;
        print_row(phase_secs as f64, concurrency, &result);
    }
    println!();
}

async fn run_deduction_flow_scenario(
    base_url: &str,
    dev_pool: &Arc<Vec<Uuid>>,
    concurrency: u32,
    duration: Duration,
) -> PhaseResult {
    let stop = Arc::new(AtomicBool::new(false));
    let total = Arc::new(AtomicU64::new(0));
    let success = Arc::new(AtomicU64::new(0));
    let fail = Arc::new(AtomicU64::new(0));
    let latencies = Arc::new(parking_lot::Mutex::new(Vec::new()));
    let client = build_client();
    let dev_pool = dev_pool.clone();

    let initiate_url = format!("{}/api/deductions/initiate", base_url);
    let confirm_url = format!("{}/api/deductions/confirm", base_url);

    let mut handles = Vec::new();
    for _ in 0..concurrency {
        let stop = stop.clone();
        let total = total.clone();
        let success = success.clone();
        let fail = fail.clone();
        let latencies = latencies.clone();
        let client = client.clone();
        let dev_pool = dev_pool.clone();
        let initiate_url = initiate_url.clone();
        let confirm_url = confirm_url.clone();

        handles.push(tokio::spawn(async move {
            loop {
                if stop.load(Ordering::Relaxed) {
                    return;
                }

                let flow_start = Instant::now();

                // 从开发者池中随机选一个
                let dev_idx = fast_random_index(dev_pool.len());
                let developer_uuid = dev_pool[dev_idx];

                // Step 1: POST initiate
                let initiate_body = serde_json::json!({
                    "developer_uuid": developer_uuid,
                    "amount": 1,
                });

                let initiate_resp = match client
                    .post(&initiate_url)
                    .json(&initiate_body)
                    .send()
                    .await
                {
                    Ok(r) => r,
                    Err(_) => {
                        total.fetch_add(1, Ordering::Relaxed);
                        fail.fetch_add(1, Ordering::Relaxed);
                        continue;
                    }
                };

                if !initiate_resp.status().is_success() {
                    total.fetch_add(1, Ordering::Relaxed);
                    fail.fetch_add(1, Ordering::Relaxed);
                    continue;
                }

                // 解析 initiate 响应获取 token
                let initiate_data: Value = match initiate_resp.json().await {
                    Ok(v) => v,
                    Err(_) => {
                        total.fetch_add(1, Ordering::Relaxed);
                        fail.fetch_add(1, Ordering::Relaxed);
                        continue;
                    }
                };

                let transaction_token = match initiate_data["data"]["transaction_token"].as_str() {
                    Some(t) => t.to_string(),
                    None => {
                        total.fetch_add(1, Ordering::Relaxed);
                        fail.fetch_add(1, Ordering::Relaxed);
                        continue;
                    }
                };

                let commit_token = match initiate_data["data"]["commit_token"].as_str() {
                    Some(t) => t.to_string(),
                    None => {
                        total.fetch_add(1, Ordering::Relaxed);
                        fail.fetch_add(1, Ordering::Relaxed);
                        continue;
                    }
                };

                // Step 2: POST confirm
                let confirm_body = serde_json::json!({
                    "transaction_token": transaction_token,
                    "commit_token": commit_token,
                });

                let confirm_resp = client
                    .post(&confirm_url)
                    .json(&confirm_body)
                    .send()
                    .await;

                let flow_elapsed = flow_start.elapsed().as_secs_f64() * 1000.0;
                total.fetch_add(1, Ordering::Relaxed);

                match confirm_resp {
                    Ok(r) if r.status().is_success() => {
                        success.fetch_add(1, Ordering::Relaxed);
                        latencies.lock().push(flow_elapsed);
                    }
                    _ => {
                        fail.fetch_add(1, Ordering::Relaxed);
                    }
                }
            }
        }));
    }

    tokio::time::sleep(duration).await;
    stop.store(true, Ordering::Relaxed);

    for h in handles {
        let _ = h.await;
    }

    let t = total.load(Ordering::Relaxed);
    let s = success.load(Ordering::Relaxed);
    let f = fail.load(Ordering::Relaxed);
    let mut lat = latencies.lock().clone();
    lat.sort_by(|a, b| a.partial_cmp(b).unwrap());

    PhaseResult {
        total: t,
        success: s,
        fail: f,
        latencies: lat,
    }
}

// ============================================================================
// Phase 3: 开发者 CRUD 流程 (Create → Get → Update → Delete)
// 每个并发任务循环执行完整 CRUD
// ============================================================================

async fn run_developer_crud_phase(
    base_url: &str,
    concurrency_levels: &[u32],
    phase_secs: u64,
) {
    println!("  ▸ POST → GET → PUT → DELETE /api/developers");
    print_header();
    let duration = Duration::from_secs(phase_secs);
    for &concurrency in concurrency_levels {
        let result = run_crud_scenario(base_url, concurrency, duration).await;
        print_row(phase_secs as f64, concurrency, &result);
    }
    println!();
}

async fn run_crud_scenario(
    base_url: &str,
    concurrency: u32,
    duration: Duration,
) -> PhaseResult {
    let stop = Arc::new(AtomicBool::new(false));
    let total = Arc::new(AtomicU64::new(0));
    let success = Arc::new(AtomicU64::new(0));
    let fail = Arc::new(AtomicU64::new(0));
    let latencies = Arc::new(parking_lot::Mutex::new(Vec::new()));
    let client = build_client();

    let create_url = format!("{}/api/developers", base_url);

    let mut handles = Vec::new();
    for _ in 0..concurrency {
        let stop = stop.clone();
        let total = total.clone();
        let success = success.clone();
        let fail = fail.clone();
        let latencies = latencies.clone();
        let client = client.clone();
        let create_url = create_url.clone();
        let base_url = base_url.to_string();

        handles.push(tokio::spawn(async move {
            loop {
                if stop.load(Ordering::Relaxed) {
                    return;
                }

                let crud_start = Instant::now();
                let dev_uuid = Uuid::new_v4();

                // Step 1: POST create
                let create_body = serde_json::json!({
                    "developer_uuid": dev_uuid,
                    "developer_name": format!("crud-test-{}", dev_uuid),
                    "risky_marks_available": 100,
                    "total_risky_marks_earned": 100,
                });

                let resp = match client.post(&create_url).json(&create_body).send().await {
                    Ok(r) => r,
                    Err(_) => {
                        total.fetch_add(1, Ordering::Relaxed);
                        fail.fetch_add(1, Ordering::Relaxed);
                        continue;
                    }
                };

                if !resp.status().is_success() {
                    total.fetch_add(1, Ordering::Relaxed);
                    fail.fetch_add(1, Ordering::Relaxed);
                    continue;
                }

                // Step 2: GET by UUID
                let dev_url = format!("{}/api/developers/{}", base_url, dev_uuid);
                let resp = match client.get(&dev_url).send().await {
                    Ok(r) => r,
                    Err(_) => {
                        total.fetch_add(1, Ordering::Relaxed);
                        fail.fetch_add(1, Ordering::Relaxed);
                        continue;
                    }
                };

                if !resp.status().is_success() {
                    total.fetch_add(1, Ordering::Relaxed);
                    fail.fetch_add(1, Ordering::Relaxed);
                    continue;
                }

                // Step 3: PUT update
                let update_body = serde_json::json!({
                    "developer_name": format!("crud-test-updated-{}", dev_uuid),
                    "successful_auths": 100,
                });
                let resp = match client.put(&dev_url).json(&update_body).send().await {
                    Ok(r) => r,
                    Err(_) => {
                        total.fetch_add(1, Ordering::Relaxed);
                        fail.fetch_add(1, Ordering::Relaxed);
                        continue;
                    }
                };

                if !resp.status().is_success() {
                    total.fetch_add(1, Ordering::Relaxed);
                    fail.fetch_add(1, Ordering::Relaxed);
                    continue;
                }

                // Step 4: DELETE
                let resp = match client.delete(&dev_url).send().await {
                    Ok(r) => r,
                    Err(_) => {
                        total.fetch_add(1, Ordering::Relaxed);
                        fail.fetch_add(1, Ordering::Relaxed);
                        continue;
                    }
                };

                let crud_elapsed = crud_start.elapsed().as_secs_f64() * 1000.0;
                total.fetch_add(1, Ordering::Relaxed);

                if resp.status().is_success() {
                    success.fetch_add(1, Ordering::Relaxed);
                    latencies.lock().push(crud_elapsed);
                } else {
                    fail.fetch_add(1, Ordering::Relaxed);
                }
            }
        }));
    }

    tokio::time::sleep(duration).await;
    stop.store(true, Ordering::Relaxed);

    for h in handles {
        let _ = h.await;
    }

    let t = total.load(Ordering::Relaxed);
    let s = success.load(Ordering::Relaxed);
    let f = fail.load(Ordering::Relaxed);
    let mut lat = latencies.lock().clone();
    lat.sort_by(|a, b| a.partial_cmp(b).unwrap());

    PhaseResult {
        total: t,
        success: s,
        fail: f,
        latencies: lat,
    }
}

/// 快速伪随机索引（不依赖 rand crate）
fn fast_random_index(len: usize) -> usize {
    if len == 0 {
        return 0;
    }
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    Instant::now().hash(&mut hasher);
    (hasher.finish() as usize) % len
}
