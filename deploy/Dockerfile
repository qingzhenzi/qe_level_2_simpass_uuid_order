# 多阶段构建
# 构建阶段
FROM rust:1.81-slim-bookworm AS builder

RUN apt-get update && apt-get install -y pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY Cargo.toml Cargo.lock* ./
COPY src/ ./src/
COPY migrations/ ./migrations/

RUN cargo build --release

# 运行阶段
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/qe_level_2_simpass_uuid_order /usr/local/bin/

EXPOSE 8080

CMD ["qe_level_2_simpass_uuid_order"]
