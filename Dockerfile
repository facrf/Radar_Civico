# syntax=docker/dockerfile:1
# Multi-stage Dockerfile para Radar Cívico

# Stage 1: Build frontend SPA (SvelteKit + Tailwind CSS)
FROM node:20-slim AS web-builder
WORKDIR /app

ARG APP_VERSION
ARG APP_COMMIT
ARG APP_COUNT
ENV APP_VERSION=$APP_VERSION
ENV APP_COMMIT=$APP_COMMIT
ENV APP_COUNT=$APP_COUNT

WORKDIR /app/web
COPY web/package*.json ./
RUN --mount=type=cache,target=/root/.npm npm ci

# Copia version.json se presente no contexto da raiz
COPY version.json* /app/
COPY web/ ./
RUN npm run check && npm run build

# Stage 2: Build Rust backend
FROM rust:slim-bookworm AS builder
WORKDIR /app

ARG APP_VERSION
ARG APP_COMMIT
ARG APP_COUNT
ENV APP_VERSION=$APP_VERSION
ENV APP_COMMIT=$APP_COMMIT
ENV APP_COUNT=$APP_COUNT

RUN apt-get -o Acquire::ForceIPv4=true update && apt-get -o Acquire::ForceIPv4=true install -y --no-install-recommends \
    pkg-config \
    build-essential \
    libssl-dev \
    git \
    && rm -rf /var/lib/apt/lists/*
COPY Cargo.toml Cargo.lock ./
COPY crates/ ./crates/
COPY tests/ ./tests/
COPY version.json* ./
RUN --mount=type=cache,id=radar-cargo-registry,target=/usr/local/cargo/registry,sharing=locked \
    --mount=type=cache,id=radar-cargo-git,target=/usr/local/cargo/git,sharing=locked \
    --mount=type=cache,id=radar-cargo-target,target=/app/target,sharing=locked \
    cargo build --locked --release -p server && cp target/release/server /app/server-binary

# Stage 3: Runner de produção
FROM debian:bookworm-slim AS runner
RUN apt-get -o Acquire::ForceIPv4=true update && apt-get -o Acquire::ForceIPv4=true install -y --no-install-recommends \
    ca-certificates \
    curl \
    sqlite3 \
    libsqlite3-0 \
    libssl3 \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY --from=builder /app/server-binary /app/server
COPY --from=web-builder /app/web/build /app/web/build

ENV DATA_DIR=/app/data
ENV PORT=8080
ENV WEB_DIR=/app/web/build
ENV RUST_LOG=info

EXPOSE 8080

VOLUME ["/app/data"]

HEALTHCHECK --interval=30s --timeout=5s --start-period=15s --retries=3 \
    CMD curl -f http://localhost:8080/health || exit 1

CMD ["/app/server"]
