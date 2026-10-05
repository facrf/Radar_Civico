# Multi-stage Dockerfile

# Stage 1: Build frontend SPA
FROM node:20-slim AS web-builder
WORKDIR /app/web
COPY web/package*.json ./
RUN npm ci
COPY web/ ./
RUN npm run build

# Stage 2: Build Rust backend
FROM rust:slim AS builder
WORKDIR /app
RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config \
    build-essential \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*
COPY . .
RUN cargo build --release -p server

# Stage 3: Runner
FROM debian:bookworm-slim AS runner
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    curl \
    sqlite3 \
    libsqlite3-0 \
    libssl3 \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY --from=builder /app/target/release/server /app/server
COPY --from=web-builder /app/web/build /app/web/build

ENV DATA_DIR=/app/data
ENV PORT=8080
ENV WEB_DIR=/app/web/build
ENV RUST_LOG=info

EXPOSE 8080

VOLUME ["/app/data"]

CMD ["/app/server"]
