#!/bin/sh
# Starts Anchored as the TrueNAS apps user (uid 568 unless PUID and PGID are set).
# DATABASE_KEY is read from the environment only. It is never stored in the image.
set -eu

PUID="${PUID:-568}"
PGID="${PGID:-568}"
PORT="${PORT:-3000}"
DATABASE_PATH="${DATABASE_PATH:-/data/anchored.db}"

case "$PUID" in
  ''|*[!0-9]*) echo "PUID must be a number." >&2; exit 1 ;;
esac
case "$PGID" in
  ''|*[!0-9]*) echo "PGID must be a number." >&2; exit 1 ;;
esac
case "$PORT" in
  ''|*[!0-9]*) echo "PORT must be a number." >&2; exit 1 ;;
esac

if [ -z "${DATABASE_KEY:-}" ] || [ "$DATABASE_KEY" = "your_database_key" ] || [ "${#DATABASE_KEY}" -lt 16 ]; then
  echo "DATABASE_KEY is required. Pass a long random string (at least 16 characters) in the container environment. Anchored will not start without it, and the key is not stored in the image." >&2
  exit 1
fi

export PORT
export DATABASE_PATH
export ANCHORED_CONTAINER=1
# Docker and Podman set HOSTNAME to the container name. Next.js listens on
# HOSTNAME, so the name would not accept the published port. Bind all interfaces.
export HOSTNAME=0.0.0.0

mkdir -p /data

drop() {
  exec setpriv --reuid "$PUID" --regid "$PGID" --clear-groups --inh-caps=-all -- "$@"
}

if [ "$(id -u)" = "0" ]; then
  if ! command -v setpriv >/dev/null 2>&1; then
    echo "setpriv is missing, so Anchored cannot switch to user ${PUID}." >&2
    exit 1
  fi
  chown "$PUID:$PGID" /data
  chmod 700 /data || true
  if ! setpriv --reuid "$PUID" --regid "$PGID" --clear-groups --inh-caps=-all -- test -w /data; then
    echo "/data is not writable by user ${PUID}:${PGID}. Mount a dataset at /data that this user can write." >&2
    exit 1
  fi
  drop node server.js
fi

if [ ! -w /data ]; then
  echo "/data is not writable. Mount a dataset at /data." >&2
  exit 1
fi
exec node server.js
