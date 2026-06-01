# UUID Deduction System

基于 Rust + Actix Web 的高性能 UUID 扣量系统，采用**两阶段提交**扣量模式，支持自动恢复机制和内存 QPS 监控。

## 功能特性

- **两阶段扣量** - 发起 -> 确认模式，带超时机制，保障事务安全
- **Redis 优先** - 扣量操作优先走 Redis，性能优先，PostgreSQL 兜底
- **自动恢复** - 按配置周期自动恢复每个开发者的扣量额度
- **内存 QPS 追踪** - 滑动窗口算法，不实时写入数据库
- **频率限制** - 基于 Redis 的按开发者限流
- **管理认证** - 管理接口通过 `X-Admin-Token` 头保护
- **扣量安全** - 可选 User-Agent 白名单和 API Token 验证
- **自动迁移** - 递归扫描 `migrations/` 目录下所有 `.sql` 文件，支持校验和验证
- **健康检查** - 定时检测 PostgreSQL 和 Redis 连接状态

## 技术栈

| 组件 | 技术 |
|-----------|------------|
| 后端 | Rust + Actix Web 4 |
| 数据库 | PostgreSQL 14+ |
| 缓存 | Redis 6+（可选，无 Redis 可降级运行） |
| 前端 | Vue.js 3 + Vite + Element Plus + ECharts |

## 快速开始

### 环境要求

- Rust 1.70+
- PostgreSQL 14+
- Redis 6+（可选）

### 后端启动

```bash
git clone <repo-url> && cd qe_level_2_simpass_uuid_order

# 配置环境变量
cp .env.example .env
# 编辑 .env，填入 PostgreSQL 连接信息

# 启动
cargo run
```

服务默认启动在 `http://0.0.0.0:8080`。启动时自动执行数据库迁移。

### 前端启动

```bash
cd vue-frontend
npm install
npm run dev
```

前端开发服务器默认启动在 `http://localhost:5173`，已配置 API 代理到后端。

## 环境变量

| 变量 | 默认值 | 说明 |
|----------|---------|-------------|
| `SERVER_HOST` | `0.0.0.0` | 绑定地址 |
| `SERVER_PORT` | `8080` | 绑定端口 |
| `PG_HOST` | `localhost` | PostgreSQL 地址 |
| `PG_PORT` | `5432` | PostgreSQL 端口 |
| `PG_USER` | `postgres` | PostgreSQL 用户名 |
| `PG_PASSWORD` | (必填) | PostgreSQL 密码 |
| `PG_DBNAME` | `postgres` | PostgreSQL 数据库名 |
| `REDIS_URL` | `redis://localhost:6379` | Redis 连接地址 |
| `REDIS_PREFIX` | `app` | Redis 键前缀 |
| `DEDUCTION_TIMEOUT_SECS` | `30` | 扣量事务超时时间 |
| `DEDUCTION_API_TOKEN` | (可选) | 扣量接口需要的 Token |
| `DEDUCTION_ALLOWED_USERAGENTS` | (可选) | 扣量接口允许的 User-Agent，逗号分隔 |
| `ADMIN_API_TOKEN` | (可选) | 管理接口 Token，通过 `X-Admin-Token` 头传递 |
| `LOG_LEVEL` | `info` | 日志级别 |

## 项目结构

```
qe_level_2_simpass_uuid_order/
├── migrations/                # SQL 迁移文件（自动递归扫描）
│   └── init/
│       ├── 001_*.sql          # __migrations 迁移追踪表
│       ├── 002_*.sql          # developers 开发者表
│       ├── 003_*.sql          # deduction_transactions 扣量事务表
│       ├── 004_*.sql          # qps_records QPS 记录表
│       ├── 005_*.sql          # request_logs 请求日志表
│       └── 006_*.sql          # system_configs 系统配置表
├── src/
│   ├── main.rs                # 入口，中间件，路由注册
│   ├── config.rs              # 环境变量配置
│   ├── errors.rs              # 统一错误处理
│   ├── db/
│   │   ├── mod.rs
│   │   ├── pg_pool.rs         # PostgreSQL 连接池
│   │   ├── redis_pool.rs      # Redis 连接管理器
│   │   ├── migrations.rs      # 自动迁移 + 校验和验证
│   │   └── health.rs          # 健康检查
│   ├── models/
│   │   ├── mod.rs
│   │   ├── response.rs        # ApiResponse<T>, PaginatedResponse<T>
│   │   ├── developer.rs       # 开发者结构体 + 创建/更新 DTO
│   │   ├── transaction.rs     # 扣量事务 + 请求 DTO
│   │   ├── qps.rs             # QPS 记录、统计响应
│   │   └── system_config.rs   # 系统配置 + CRUD DTO
│   ├── handlers/
│   │   ├── mod.rs
│   │   ├── developer.rs       # 开发者 CRUD
│   │   ├── deduction.rs       # 发起/确认/取消扣量
│   │   ├── qps.rs             # 实时 QPS、历史、统计
│   │   └── system_config.rs   # 系统配置 CRUD
│   ├── services/
│   │   ├── mod.rs
│   │   ├── deduction.rs       # 扣量业务逻辑
│   │   ├── qps.rs             # 滑动窗口 QPS 追踪器
│   │   └── system_config.rs   # 系统配置业务逻辑
│   ├── repositories/
│   │   ├── mod.rs
│   │   ├── developer.rs       # 开发者数据库访问
│   │   └── transaction.rs     # 事务数据库访问
│   ├── middleware/
│   │   ├── mod.rs
│   │   ├── auth.rs            # 扣量接口认证中间件
│   │   └── rate_limiter.rs    # 按开发者限流中间件
│   ├── cache/
│   │   ├── mod.rs
│   │   └── redis.rs           # Redis 缓存辅助
│   └── tasks/
│       ├── mod.rs
│       ├── expiration.rs      # 过期事务清理
│       └── recovery.rs        # 周期扣量恢复
├── vue-frontend/              # Vue 3 管理端 UI
│   └── src/
│       ├── api/index.js       # Axios API 客户端
│       ├── views/             # 仪表盘、开发者管理、扣量管理、QPS 监控
│       └── router/index.js    # 路由配置
├── .env.example               # 环境变量模板
└── Cargo.toml
```

## API 接口

所有响应使用统一格式：

```json
{
  "code": "SUCCESS",
  "message": "ok",
  "data": { ... }
}
```

分页响应：

```json
{
  "code": "SUCCESS",
  "message": "ok",
  "data": {
    "data": [ ... ],
    "total": 100,
    "page": 1,
    "page_size": 20
  }
}
```

错误响应：

```json
{
  "code": "NOT_FOUND",
  "message": "Developer not found"
}
```

### 健康检查

```
GET /health
```

```bash
curl http://localhost:8080/health
# {"status":"healthy"}
```

### 开发者管理

#### 创建开发者

```
POST /api/developers

X-Admin-Token: <token>     （如果配置了 ADMIN_API_TOKEN）
```

```bash
curl -X POST http://localhost:8080/api/developers \
  -H "Content-Type: application/json" \
  -d '{
    "developer_name": "MyApp",
    "deduction_available": 100,
    "deduction_limit": 1000,
    "rate_limit_per_second": 50,
    "recovery_amount": 10,
    "recovery_interval_secs": 60
  }'
```

**请求字段说明：**

| 字段 | 类型 | 必填 | 说明 |
|--------|------|------|-------------|
| developer_uuid | UUID | 否 | 不传自动生成 |
| developer_name | String | 是 | 开发者名称 |
| successful_auths | Integer | 否 | 成功认证次数（默认 0） |
| deduction_available | Integer | 否 | 当前可用扣量次数（默认 0） |
| deduction_limit | Integer | 否 | 最大扣量上限（默认 1000） |
| rate_limit_per_second | Integer | 否 | 每秒请求限制（默认 100） |
| recovery_amount | Integer | 否 | 每次恢复的扣量数（默认 10） |
| recovery_interval_secs | Integer | 否 | 恢复间隔秒数（默认 60） |

**响应：**

```json
{
  "code": "SUCCESS",
  "data": { "developer_uuid": "e2646e16-..." }
}
```

#### 开发者列表

```
GET /api/developers?page=1&page_size=20&search=<名称>

X-Admin-Token: <token>
```

| 参数 | 类型 | 说明 |
|--------|------|-------------|
| page | Integer | 页码（默认 1） |
| page_size | Integer | 每页条数（默认 20） |
| search | String | 按名称模糊搜索 |

```bash
curl -H "X-Admin-Token: your-admin-token" \
  "http://localhost:8080/api/developers?page=1&page_size=10"
curl -H "X-Admin-Token: your-admin-token" \
  "http://localhost:8080/api/developers?search=MyApp"
```

#### 获取开发者

```
GET /api/developers/{uuid}
```

```bash
curl http://localhost:8080/api/developers/e2646e16-...
```

#### 更新开发者

```
PUT /api/developers/{uuid}
```

字段均为可选，只更新提供的字段。

```bash
curl -X PUT http://localhost:8080/api/developers/e2646e16-... \
  -H "Content-Type: application/json" \
  -d '{
    "deduction_limit": 2000,
    "rate_limit_per_second": 100
  }'
```

#### 删除开发者

```
DELETE /api/developers/{uuid}
```

会级联删除关联的扣量事务。

```bash
curl -X DELETE http://localhost:8080/api/developers/e2646e16-...
```

### 扣量接口（两阶段提交）

扣量接口需要认证（如果已配置），详见[安全](#安全)章节。

#### 阶段一：发起扣量

```
POST /api/deductions/initiate

请求头（如果配置了认证）：
  X-API-Token: <token>
  User-Agent: <允许的 User-Agent>
```

```bash
curl -X POST http://localhost:8080/api/deductions/initiate \
  -H "Content-Type: application/json" \
  -H "X-API-Token: your-secret-token" \
  -H "User-Agent: MyApp/1.0" \
  -d '{
    "developer_uuid": "e2646e16-...",
    "amount": 5
  }'
```

响应中包含 `transaction_token`、`commit_token` 和 `expires_at`，必须在超时前（默认 30 秒）完成确认。

```json
{
  "code": "SUCCESS",
  "message": "ok",
  "data": {
    "transaction_token": "c5235bb3-...",
    "commit_token": "2f6d29cf-...",
    "expires_at": "2026-06-01T05:51:14Z"
  }
}
```

#### 阶段二：确认扣量

```
POST /api/deductions/confirm
```

```bash
curl -X POST http://localhost:8080/api/deductions/confirm \
  -H "Content-Type: application/json" \
  -H "X-API-Token: your-secret-token" \
  -d '{
    "transaction_token": "c5235bb3-...",
    "commit_token": "2f6d29cf-..."
  }'
```

#### 取消扣量

```
POST /api/deductions/cancel
```

```bash
curl -X POST http://localhost:8080/api/deductions/cancel \
  -H "Content-Type: application/json" \
  -d '{
    "transaction_token": "c5235bb3-..."
  }'
```

#### 扣量事务列表

```
GET /api/deductions/transactions?page=1&page_size=20&developer_uuid=<uuid>&status=pending

X-Admin-Token: <token>
```

| 参数 | 类型 | 说明 |
|--------|------|-------------|
| page | Integer | 页码 |
| page_size | Integer | 每页条数 |
| developer_uuid | UUID | 按开发者过滤 |
| status | String | 按状态过滤：pending/committed/cancelled/expired |

```bash
curl -H "X-Admin-Token: your-admin-token" \
  "http://localhost:8080/api/deductions/transactions?page=1&page_size=20"
curl -H "X-Admin-Token: your-admin-token" \
  "http://localhost:8080/api/deductions/transactions?developer_uuid=e2646e16-...&status=committed"
```

#### 获取单笔扣量

```
GET /api/deductions/transactions/{token}
```

```bash
curl http://localhost:8080/api/deductions/transactions/c5235bb3-...
```

### QPS 监控

QPS 使用**内存滑动窗口**追踪，不实时写数据库。

#### 实时 QPS

```
GET /api/qps/current?api_path=/api/developers

X-Admin-Token: <token>
```

```bash
curl -H "X-Admin-Token: your-admin-token" \
  "http://localhost:8080/api/qps/current?api_path=/api/developers"
```

```json
{
  "code": "SUCCESS",
  "data": {
    "current_qps": 15,
    "avg_qps_1m": 12.5,
    "avg_qps_5m": 8.3
  }
}
```

#### QPS 统计

```
GET /api/qps/stats

X-Admin-Token: <token>
```

```bash
curl -H "X-Admin-Token: your-admin-token" \
  http://localhost:8080/api/qps/stats
```

```json
{
  "code": "SUCCESS",
  "data": {
    "current_qps": 15,
    "avg_qps_1m": 12.5,
    "avg_qps_5m": 8.3,
    "avg_qps_1h": 5.1,
    "total_requests": 15234,
    "api_stats": [
      { "api_path": "/api/developers", "count": 450, "qps": 7.5 },
      { "api_path": "/api/deductions/initiate", "count": 180, "qps": 3.0 }
    ]
  }
}
```

#### QPS 历史

```
GET /api/qps/history?api_path=/api/developers&minutes=10&page_size=50

X-Admin-Token: <token>
```

从 `qps_records` 表查询（后台任务定时聚合写入）。

```bash
curl -H "X-Admin-Token: your-admin-token" \
  "http://localhost:8080/api/qps/history?minutes=30&page_size=100"
```

### 系统配置

#### 配置列表

```
GET /api/system/configs

X-Admin-Token: <token>
```

```bash
curl -H "X-Admin-Token: your-admin-token" \
  http://localhost:8080/api/system/configs
```

#### 获取配置

```
GET /api/system/configs/{key}
```

```bash
curl http://localhost:8080/api/system/configs/deduction_allowed_useragents
```

#### 创建配置

```
POST /api/system/configs
```

```bash
curl -X POST http://localhost:8080/api/system/configs \
  -H "Content-Type: application/json" \
  -d '{
    "config_key": "my_setting",
    "config_value": "my_value",
    "description": "可选描述"
  }'
```

#### 更新配置

```
PUT /api/system/configs/{key}
```

```bash
curl -X PUT http://localhost:8080/api/system/configs/my_setting \
  -H "Content-Type: application/json" \
  -d '{
    "config_value": "new_value"
  }'
```

#### 删除配置

```
DELETE /api/system/configs/{key}
```

```bash
curl -X DELETE http://localhost:8080/api/system/configs/my_setting
```

## 安全

### 管理接口认证

以下接口在设置了 `ADMIN_API_TOKEN` 环境变量后，需要通过 `X-Admin-Token` 头传递 Token：

```
GET    /api/developers
POST   /api/developers
PUT    /api/developers/{uuid}
DELETE /api/developers/{uuid}
GET    /api/deductions/transactions
GET    /api/qps/current
GET    /api/qps/history
GET    /api/qps/stats
GET    /api/system/configs
POST   /api/system/configs
PUT    /api/system/configs/{key}
DELETE /api/system/configs/{key}
```

```bash
curl -H "X-Admin-Token: your-admin-token" http://localhost:8080/api/developers
```

### 扣量接口认证

扣量接口可以通过以下两种方式保护（按优先级检查）：

1. **环境变量**（最高优先级）：
   - `DEDUCTION_ALLOWED_USERAGENTS` -- 允许的 User-Agent 列表（逗号分隔）
   - `DEDUCTION_API_TOKEN` -- 要求请求携带 `X-API-Token` 头
2. **数据库配置**（通过 `system_configs` 表）：
   - `deduction_allowed_useragents`
   - `deduction_api_token`

如果均未配置，扣量接口开放访问。

```bash
# 同时校验 Token 和 User-Agent：
curl -X POST http://localhost:8080/api/deductions/initiate \
  -H "Content-Type: application/json" \
  -H "X-API-Token: your-deduction-token" \
  -H "User-Agent: MyApp/1.0" \
  -d '{"developer_uuid":"...","amount":5}'
```

## 扣量流程

采用**两阶段提交**模式保障数据一致性：

```
  客户端                    服务端                    Redis + PostgreSQL
    |                          |                            |
    |-- POST /initiate ------>|                            |
    |                         |-- 预扣余额 --------------->|
    |                         |-- 创建事务 --------------->|
    |<-- { tx_token, ct } ---|                            |
    |                          |                            |
    | （在超时时间内）        |                            |
    |                          |                            |
    |-- POST /confirm ------->|                            |
    |                         |-- 验证 commit_token ------->|
    |                         |-- 扣除余额 --------------->|
    |                         |-- 标记已确认 -------------->|
    |<-- success -------------|                            |
```

- **发起**：在 Redis 中预扣金额，创建待确认事务，生成唯一的 `commit_token`
- **确认**：需要同时提供 `transaction_token` 和 `commit_token`，完成最终扣量
- **取消**：释放预扣金额，标记事务为已取消
- **过期**：后台定时任务清理超时的待确认事务

## 恢复机制

每个开发者拥有自动的扣量额度恢复：

- `recovery_amount` -- 每次恢复的扣量数量
- `recovery_interval_secs` -- 恢复间隔（秒）
- `deduction_limit` -- 扣量上限（`deduction_available` 不会超过此值）

恢复以后台任务运行，每 10 秒检查一次：

```
new_available = min(current_available + recovery_amount, deduction_limit)
```

## 迁移系统

服务启动时**自动执行**数据库迁移：

1. 递归扫描 `migrations/` 目录下所有 `.sql` 文件
2. 按文件路径排序（确保 `init/001_*.sql` 先于 `init/002_*.sql` 执行）
3. 计算合并 SQL 内容的 SHA256 校验和
4. 查询 `__migrations` 表确认当前版本
5. 如果未执行过，则执行所有 SQL 语句
6. 记录迁移信息：版本号、校验和、执行耗时

```sql
-- 查看迁移历史
SELECT version, name, checksum, execution_time_ms, applied_at
FROM sl_uuid.__migrations ORDER BY version DESC;
```

## 数据库表结构

### `__migrations`（迁移追踪表）

| 字段 | 类型 | 说明 |
|--------|------|-------------|
| version | BIGINT | 迁移版本（主键） |
| name | VARCHAR(255) | 迁移名称 |
| checksum | VARCHAR(64) | SHA256 校验和 |
| execution_time_ms | BIGINT | 执行耗时 |
| applied_at | TIMESTAMPTZ | 执行时间 |
| applied_by | VARCHAR(255) | 数据库用户 |

### `developers`（开发者表）

| 字段 | 类型 | 默认值 | 说明 |
|--------|------|---------|-------------|
| developer_uuid | UUID | 主键 | 唯一标识 |
| developer_name | VARCHAR(255) | | 开发者名称（唯一） |
| successful_auths | BIGINT | 0 | 认证次数 |
| deduction_available | INT | 0 | 当前可用扣量次数 |
| deduction_limit | INT | 1000 | 扣量上限 |
| recovery_amount | INT | 10 | 每次恢复的扣量数 |
| recovery_interval_secs | INT | 60 | 恢复间隔 |
| rate_limit_per_second | INT | 100 | 每秒请求限制 |
| last_recovery_time | TIMESTAMPTZ | NULL | 上次恢复时间 |
| create_time | TIMESTAMPTZ | NOW() | 创建时间 |
| updated_at | TIMESTAMPTZ | NOW() | 更新时间 |

### `deduction_transactions`（扣量事务表）

| 字段 | 类型 | 说明 |
|--------|------|-------------|
| id | BIGSERIAL | 自增主键 |
| developer_uuid | UUID | 外键 -> developers |
| transaction_token | UUID | 事务唯一标识 |
| amount | INT | 扣量数量（> 0） |
| status | VARCHAR(20) | pending/committed/cancelled/expired |
| commit_token | UUID | 确认凭证 |
| created_at | TIMESTAMPTZ | 创建时间 |
| expires_at | TIMESTAMPTZ | 超时时间 |
| confirmed_at | TIMESTAMPTZ | 确认时间 |

### `request_logs`（请求日志表）

| 字段 | 类型 | 说明 |
|--------|------|-------------|
| id | BIGSERIAL | 自增主键 |
| developer_uuid | UUID | 开发者（可为空） |
| api_path | VARCHAR(255) | 请求路径 |
| method | VARCHAR(10) | GET/POST/PUT/DELETE/PATCH |
| status_code | INT | HTTP 状态码 |
| processed_at | TIMESTAMPTZ | 处理时间 |
| latency_ms | INT | 响应耗时 |
| client_ip | INET | 客户端 IP |

### `qps_records`（QPS 记录表）

由后台任务定时聚合写入，不按请求实时写入。

| 字段 | 类型 | 说明 |
|--------|------|-------------|
| recorded_at | TIMESTAMPTZ | 记录时间 |
| total_qps | INT | QPS 值 |
| api_path | VARCHAR(255) | API 路径 |
| developer_uuid | UUID | 开发者（可为空） |

### `system_configs`（系统配置表）

| 字段 | 类型 | 说明 |
|--------|------|-------------|
| config_key | VARCHAR(255) | 配置键（唯一） |
| config_value | TEXT | 配置值 |
| description | TEXT | 说明 |
| created_at | TIMESTAMPTZ | 创建时间 |
| updated_at | TIMESTAMPTZ | 更新时间 |

## 调用示例

### curl

```bash
# 健康检查
curl http://localhost:8080/health

# 创建开发者
curl -X POST http://localhost:8080/api/developers \
  -H "Content-Type: application/json" \
  -H "X-Admin-Token: your-admin-token" \
  -d '{"developer_name":"DemoApp","deduction_available":100,"deduction_limit":1000,"rate_limit_per_second":50,"recovery_amount":10,"recovery_interval_secs":60}'

# 发起扣量
curl -X POST http://localhost:8080/api/deductions/initiate \
  -H "Content-Type: application/json" \
  -H "X-API-Token: your-deduction-token" \
  -H "User-Agent: MyApp/1.0" \
  -d '{"developer_uuid":"e2646e16-...","amount":5}'

# 确认扣量
curl -X POST http://localhost:8080/api/deductions/confirm \
  -H "Content-Type: application/json" \
  -H "X-API-Token: your-deduction-token" \
  -d '{"transaction_token":"c5235bb3-...","commit_token":"2f6d29cf-..."}'

# 扣量事务列表
curl -H "X-Admin-Token: your-admin-token" \
  "http://localhost:8080/api/deductions/transactions?page=1&page_size=10"

# QPS 统计
curl -H "X-Admin-Token: your-admin-token" \
  http://localhost:8080/api/qps/stats
```

### Python

```python
import requests

BASE = "http://localhost:8080"
ADMIN_TOKEN = "your-admin-token"
DEDUCTION_TOKEN = "your-deduction-token"

admin_headers = {"X-Admin-Token": ADMIN_TOKEN}
deduct_headers = {
    "X-API-Token": DEDUCTION_TOKEN,
    "User-Agent": "MyApp/1.0",
}

# 创建开发者
resp = requests.post(f"{BASE}/api/developers",
    headers=admin_headers, json={
        "developer_name": "DemoApp",
        "deduction_available": 100,
        "deduction_limit": 1000,
    })
dev_uuid = resp.json()["data"]["developer_uuid"]
print(f"开发者: {dev_uuid}")

# 开发者列表
devs = requests.get(f"{BASE}/api/developers?page=1",
    headers=admin_headers).json()
print(f"共 {devs['data']['total']} 个开发者")

# 获取开发者详情
dev = requests.get(f"{BASE}/api/developers/{dev_uuid}").json()
print(f"可用扣量: {dev['data']['deduction_available']}")

# 发起扣量
tx = requests.post(f"{BASE}/api/deductions/initiate",
    headers=deduct_headers, json={
        "developer_uuid": dev_uuid,
        "amount": 5,
    }).json()["data"]
print(f"事务 Token: {tx['transaction_token']}")

# 确认扣量
resp = requests.post(f"{BASE}/api/deductions/confirm",
    headers=deduct_headers, json={
        "transaction_token": tx["transaction_token"],
        "commit_token": tx["commit_token"],
    })
print(resp.json()["message"])

# 取消扣量
resp = requests.post(f"{BASE}/api/deductions/cancel",
    headers=deduct_headers, json={
        "transaction_token": tx["transaction_token"],
    })
print(resp.json()["message"])

# QPS 统计
qps = requests.get(f"{BASE}/api/qps/stats",
    headers=admin_headers).json()
print(f"当前 QPS: {qps['data']['current_qps']}")

# 系统配置列表
configs = requests.get(f"{BASE}/api/system/configs",
    headers=admin_headers).json()
print(configs)
```

### PHP

```php
<?php

$base = "http://localhost:8080";
$adminToken = "your-admin-token";
$deductionToken = "your-deduction-token";

function api_call($method, $url, $headers = [], $body = null) {
    $ch = curl_init($url);
    curl_setopt($ch, CURLOPT_CUSTOMREQUEST, $method);
    curl_setopt($ch, CURLOPT_RETURNTRANSFER, true);
    curl_setopt($ch, CURLOPT_HTTPHEADER, $headers);
    if ($body) {
        curl_setopt($ch, CURLOPT_POSTFIELDS, json_encode($body));
    }
    $result = curl_exec($ch);
    curl_close($ch);
    return json_decode($result, true);
}

$adminHeaders = [
    "Content-Type: application/json",
    "X-Admin-Token: $adminToken",
];

// 创建开发者
$resp = api_call("POST", "$base/api/developers", $adminHeaders, [
    "developer_name" => "DemoApp",
    "deduction_available" => 100,
    "deduction_limit" => 1000,
    "rate_limit_per_second" => 50,
    "recovery_amount" => 10,
    "recovery_interval_secs" => 60,
]);
$devUuid = $resp["data"]["developer_uuid"];
echo "开发者: $devUuid\n";

// 获取开发者
$dev = api_call("GET", "$base/api/developers/$devUuid", $adminHeaders);
echo "可用扣量: " . $dev["data"]["deduction_available"] . "\n";

// 更新开发者
api_call("PUT", "$base/api/developers/$devUuid", $adminHeaders, [
    "deduction_limit" => 2000,
]);

// 发起扣量
$deductHeaders = [
    "Content-Type: application/json",
    "X-API-Token: $deductionToken",
    "User-Agent: MyApp/1.0",
];
$tx = api_call("POST", "$base/api/deductions/initiate", $deductHeaders, [
    "developer_uuid" => $devUuid,
    "amount" => 5,
]);
echo "事务 Token: " . $tx["data"]["transaction_token"] . "\n";

// 确认扣量
$confirm = api_call("POST", "$base/api/deductions/confirm", $deductHeaders, [
    "transaction_token" => $tx["data"]["transaction_token"],
    "commit_token" => $tx["data"]["commit_token"],
]);
echo $confirm["message"] . "\n";

// 事务列表
$txs = api_call("GET", "$base/api/deductions/transactions?page=1&page_size=5", $adminHeaders);
echo "事务总数: " . $txs["data"]["total"] . "\n";

// QPS 统计
$qps = api_call("GET", "$base/api/qps/stats", $adminHeaders);
echo "当前 QPS: " . $qps["data"]["current_qps"] . "\n";
```

### Rust

```rust
use reqwest::{Client, header};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let base = "http://localhost:8080";
    let admin_token = "your-admin-token";
    let deduction_token = "your-deduction-token";

    let client = Client::new();

    // ---- 健康检查 ----
    let resp = client.get(format!("{}/health", base))
        .send().await?;
    println!("健康状态: {}", resp.text().await?);

    // ---- 创建开发者 ----
    let admin_headers = {
        let mut h = header::HeaderMap::new();
        h.insert("X-Admin-Token", header::HeaderValue::from_static(admin_token));
        h
    };

    let create_body = serde_json::json!({
        "developer_name": "DemoApp",
        "deduction_available": 100,
        "deduction_limit": 1000,
        "rate_limit_per_second": 50,
    });

    let resp: serde_json::Value = client.post(format!("{}/api/developers", base))
        .headers(admin_headers.clone())
        .json(&create_body)
        .send().await?
        .json().await?;

    let dev_uuid = resp["data"]["developer_uuid"].as_str().unwrap();
    println!("开发者: {}", dev_uuid);

    // ---- 发起扣量 ----
    let deduct_headers = {
        let mut h = header::HeaderMap::new();
        h.insert("X-API-Token", header::HeaderValue::from_static(deduction_token));
        h.insert("User-Agent", header::HeaderValue::from_static("MyApp/1.0"));
        h
    };

    let initiate_body = serde_json::json!({
        "developer_uuid": dev_uuid,
        "amount": 5,
    });

    let tx: serde_json::Value = client.post(format!("{}/api/deductions/initiate", base))
        .headers(deduct_headers.clone())
        .json(&initiate_body)
        .send().await?
        .json().await?;

    println!("事务 Token: {}", tx["data"]["transaction_token"]);

    // ---- 确认扣量 ----
    let confirm_body = serde_json::json!({
        "transaction_token": tx["data"]["transaction_token"],
        "commit_token": tx["data"]["commit_token"],
    });

    let confirm: serde_json::Value = client.post(format!("{}/api/deductions/confirm", base))
        .headers(deduct_headers.clone())
        .json(&confirm_body)
        .send().await?
        .json().await?;

    println!("确认结果: {}", confirm["message"]);

    // ---- QPS 统计 ----
    let qps: serde_json::Value = client.get(format!("{}/api/qps/stats", base))
        .headers(admin_headers.clone())
        .send().await?
        .json().await?;

    println!("当前 QPS: {}", qps["data"]["current_qps"]);

    Ok(())
}
```

## 生产构建

```bash
cargo build --release
./target/release/qe_level_2_simpass_uuid_order
```

Release 构建启用了 LTO 和单 codegen unit 以优化性能。
