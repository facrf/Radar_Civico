# Multi-stage Dockerfile
FROM rust:slim AS builder

WORKDIR /app

# Install build dependencies
RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config \
    build-essential \
    && rm -rf /var/lib/apt/lists/*

COPY . .

RUN cargo build --release -p server

# Runner stage
FROM debian:bookworm-slim AS runner

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    sqlite3 \
    libsqlite3-0 \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY --from=builder /app/target/release/server /app/server

ENV DATA_DIR=/app/data
ENV PORT=8080

EXPOSE 8080

VOLUME ["/app/data"]

CMD ["/app/server"]
