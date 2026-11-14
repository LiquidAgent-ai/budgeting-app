# Multi-stage Rust build. One image for Podman and Docker (linux/amd64).
# DATABASE_KEY is never baked into the image.

FROM rust:1.100-bookworm AS build
WORKDIR /src
RUN apt-get update \
  && apt-get install -y --no-install-recommends pkg-config libssl-dev \
  && rm -rf /var/lib/apt/lists/*
COPY Cargo.toml Cargo.lock ./
COPY src ./src
COPY static ./static
RUN cargo build --release \
  && test -x target/release/anchored

FROM debian:bookworm-slim AS runner
WORKDIR /app
ENV PORT=3000 \
    DATABASE_PATH=/data/anchored.db \
    ANCHORED_CONTAINER=1
# DATABASE_KEY is not set here. Pass it when the container starts.
RUN apt-get update \
  && apt-get install -y --no-install-recommends ca-certificates curl util-linux \
  && rm -rf /var/lib/apt/lists/* \
  && mkdir -p /data \
  && chown 568:568 /data \
  && chmod 700 /data
COPY --from=build /src/target/release/anchored /app/anchored
COPY static /app/static
COPY docker/entrypoint.sh /entrypoint.sh
RUN chmod 755 /entrypoint.sh /app/anchored
EXPOSE 3000
HEALTHCHECK --interval=30s --timeout=5s --start-period=15s --retries=3 \
  CMD curl -fsS "http://127.0.0.1:${PORT:-3000}/api/health" >/dev/null
ENTRYPOINT ["/entrypoint.sh"]
