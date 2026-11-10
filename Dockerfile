# glibc Node. SQLCipher's native module does not ship a musl build, so this is
# bookworm-slim and not Alpine. One image, usable by Podman and Docker.

FROM node:22-bookworm-slim AS deps
WORKDIR /app
ENV ELECTRON_SKIP_BINARY_DOWNLOAD=1
RUN apt-get update \
  && apt-get install -y --no-install-recommends python3 make g++ \
  && rm -rf /var/lib/apt/lists/*
COPY package.json package-lock.json ./
RUN npm ci

FROM node:22-bookworm-slim AS build
WORKDIR /app
ENV ELECTRON_SKIP_BINARY_DOWNLOAD=1 \
    NEXT_TELEMETRY_DISABLED=1
COPY --from=deps /app/node_modules ./node_modules
COPY . .
RUN npm run build \
  && test -f .next/standalone/server.js \
  && test -d .next/standalone/node_modules/better-sqlite3-multiple-ciphers

FROM node:22-bookworm-slim AS runner
WORKDIR /app
ENV NODE_ENV=production \
    NEXT_TELEMETRY_DISABLED=1 \
    PORT=3000 \
    DATABASE_PATH=/data/anchored.db \
    ANCHORED_CONTAINER=1
# DATABASE_KEY is not set here. Pass it when the container starts.
COPY --from=build /app/.next/standalone ./
COPY --from=build /app/.next/static ./.next/static
# Standalone tracing omits Next files the server still loads (for example
# dist/lib/metadata). The Windows package does the same replacement.
RUN rm -rf node_modules/next
COPY --from=build /app/node_modules/next ./node_modules/next
COPY docker/entrypoint.sh /entrypoint.sh
RUN apt-get update \
  && apt-get install -y --no-install-recommends util-linux \
  && rm -rf /var/lib/apt/lists/* \
  && chmod 755 /entrypoint.sh \
  && mkdir -p /data \
  && chown 568:568 /data \
  && chmod 700 /data
EXPOSE 3000
HEALTHCHECK --interval=30s --timeout=5s --start-period=20s --retries=3 \
  CMD node -e "fetch('http://127.0.0.1:'+(process.env.PORT||3000)+'/api/health').then((r)=>process.exit(r.ok?0:1)).catch(()=>process.exit(1))"
ENTRYPOINT ["/entrypoint.sh"]
