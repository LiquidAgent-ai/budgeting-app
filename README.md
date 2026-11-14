# Anchored

Anchored is a personal finance app that stays on your computer. The server is written in Rust. Your books live in an encrypted SQLite file. Bank linking, when configured, is read-only through Plaid.

You can run it locally, or in one container with Podman or Docker (including on TrueNAS). The published image is `ghcr.io/liquidagent-ai/budgeting-app`.

## Run locally

```bash
cp .env.example .env.local
# Edit .env.local and set DATABASE_KEY to a long random string.

cargo run --release
```

Open http://127.0.0.1:3000. Local runs listen on 127.0.0.1 only. The first start creates `data/anchored.db` and loads a demo household.

```bash
# Re-seed demo data (wipes the database file)
curl -X POST http://127.0.0.1:3000/api/seed -H 'content-type: application/json' -d '{"fresh":true}'
```

## The database key

Anchored will not create or open the database when the key is missing or wrong.

For local runs and containers, put a long random string in `.env.local` as `DATABASE_KEY`, or pass it in the process environment. Keep that same string somewhere safe. Do not put the key in the image. If `DATABASE_KEY` is missing or different from the one that encrypted the file, Anchored refuses to open it. If you lose the string, the data cannot be recovered.

## Run it in a container

Pull the published image (Podman or Docker):

```bash
podman pull ghcr.io/liquidagent-ai/budgeting-app:latest
# or: docker pull ghcr.io/liquidagent-ai/budgeting-app:latest
```

Or build from this folder:

```bash
podman build -t anchored .
# or: docker build -t anchored .
```

Start it. Replace the dataset path and the key:

```bash
podman run -d --name anchored \
  --restart unless-stopped \
  -p 3000:3000 \
  -v /mnt/tank/anchored:/data \
  -e DATABASE_KEY='paste-a-long-random-string' \
  -e PUID=568 \
  -e PGID=568 \
  ghcr.io/liquidagent-ai/budgeting-app:latest
```

Inside the container, Anchored listens on port 3000 so the published port works. The file is stored at `/data/anchored.db` and written as user 568 (TrueNAS apps user) unless you change `PUID` / `PGID`.

### Compose

Create a file named `.env` (do not commit it):

```text
DATABASE_KEY=paste-a-long-random-string
ANCHORED_DATA=/mnt/tank/anchored
```

```bash
podman compose up -d --build
# or: docker compose up -d --build
```

A check of the server is `GET /api/health`.

## Bank linking

To link a bank, set `PLAID_CLIENT_ID`, `PLAID_SECRET`, and optional `PLAID_ENV=sandbox` in `.env.local` or the container environment. Anchored only asks Plaid for read-only access: transactions, balances, investments, and optional liabilities. It does not request transfers, payments, Auth, or account numbers for moving money.

## Logs

Anchored writes one plain line when something actually happens: the server starts, the database opens or closes, a bank is linked, a sync finishes or fails, or a request fails. Successful pages, static files, and `/api/health` checks are left out. The database key, bank tokens, account numbers, and transaction details are not written.

## Commands

| Command | What it does |
| --- | --- |
| `cargo run` | Website at http://127.0.0.1:3000 |
| `cargo run --release` | Optimized local binary |
| `cargo build --release` | Build `target/release/anchored` |
| `podman build -t anchored .` | Build the container image |
| `docker build -t anchored .` | Same with Docker |
