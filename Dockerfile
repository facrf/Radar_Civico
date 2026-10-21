# syntax=docker/dockerfile:1
# Multi-stage Dockerfile para Radar Cívico

# Stage 1: Build frontend SPA (SvelteKit + Tailwind CSS)
FROM --platform=$BUILDPLATFORM node:22-slim AS web-builder
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

ARG TARGETARCH
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
# COPY preserva mtimes: fontes copiadas podem ser mais antigas que um binário do
# target compartilhado. Atualizar os mtimes força o Cargo a validar o código local.
RUN --mount=type=cache,id=radar-cargo-registry,target=/usr/local/cargo/registry,sharing=locked \
    --mount=type=cache,id=radar-cargo-git,target=/usr/local/cargo/git,sharing=locked \
    --mount=type=cache,id=radar-cargo-target-${TARGETARCH:-amd64},target=/app/target,sharing=locked \
    find crates tests -type f -exec touch {} + && touch Cargo.toml Cargo.lock && \
    cargo build --locked --release -p server && cp target/release/server /app/server-binary

# Stage 3: Runner de produção
FROM debian:bookworm-slim AS runner
RUN apt-get -o Acquire::ForceIPv4=true update && apt-get -o Acquire::ForceIPv4=true install -y --no-install-recommends \
    ca-certificates \
    curl \
    sqlite3 \
    libsqlite3-0 \
    libssl3 \
    gosu \
    && rm -rf /var/lib/apt/lists/*

RUN groupadd -g 10001 radar && useradd -u 10001 -g radar -s /bin/sh -d /app radar

WORKDIR /app

COPY --from=builder /app/server-binary /app/server
COPY --from=web-builder /app/web/build /app/web/build
COPY scripts/docker-entrypoint.sh /app/docker-entrypoint.sh
RUN chmod +x /app/docker-entrypoint.sh && mkdir -p /app/data && chown -R radar:radar /app

ENV DATA_DIR=/app/data
ENV PORT=8080
ENV HOST=0.0.0.0
ENV WEB_DIR=/app/web/build
ENV RUST_LOG=info

EXPOSE 8080

VOLUME ["/app/data"]

HEALTHCHECK --interval=30s --timeout=5s --start-period=15s --retries=3 \
    CMD curl -f http://localhost:8080/health || exit 1

ENTRYPOINT ["/app/docker-entrypoint.sh"]
CMD ["/app/server"]
