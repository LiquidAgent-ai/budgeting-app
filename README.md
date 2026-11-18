# Anchored

Anchored is a personal finance app that stays on your computer. The server is written in Rust. Your books live in an encrypted SQLite file. You can import bank and card exports (CSV, TSV, OFX, QFX, QIF) without linking an institution. Bank linking, when configured, is read-only through Teller (free developer tier) or Plaid.

You can run it locally, or in one container with Podman or Docker (including on TrueNAS). The published image is `ghcr.io/liquidagent-ai/budgeting-app`.

## Windows

Download the latest `Anchored-Setup` installer from [GitHub Releases](https://github.com/LiquidAgent-ai/budgeting-app/releases). Double-click it. If Windows says the app is unrecognized, choose More info, then Run anyway.

The installer adds Anchored to the Start menu and the desktop. Open that shortcut. Anchored opens its own window.

The books stay encrypted at `%APPDATA%\Anchored\anchored.db`. Windows seals the database key for your account in `%APPDATA%\Anchored\db-key.bin`, so you do not set `DATABASE_KEY`. A copy of the database file on its own cannot be read. Uninstalling Anchored leaves that folder in place.

To link a bank with Teller, put `TELLER_APPLICATION_ID` and optional `TELLER_ENV` in `%APPDATA%\Anchored\teller.env`. For Teller development/production, also set `TELLER_CERT_PATH` and `TELLER_KEY_PATH` to certificate files under that folder. Plaid remains available via `%APPDATA%\Anchored\plaid.env`. Anchored only asks for read-only access.

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

## Import statements (no bank API)

Open **Import** in the app, pick a manual or linked account, and upload a CSV, TSV, OFX, QFX, or QIF export from your bank or card. Re-importing the same rows skips duplicates. This path never talks to Teller or Plaid.

You can also post a multipart form to `POST /api/import` with fields `account_id` and `file`.

## Bank linking

### Teller (free for small personal use)

Create an application at [teller.io](https://teller.io/), then set:

- `TELLER_APPLICATION_ID`
- optional `TELLER_ENV=sandbox` (default), `development`, or `production`
- for development/production: `TELLER_CERT_PATH` + `TELLER_KEY_PATH`, or `TELLER_CERT_PEM` + `TELLER_KEY_PEM`

On Accounts, use **Connect a bank**. Anchored requests only `balance` and `transactions`. It refuses payments, verify, and identity (`src/teller/mod.rs`).

Teller's developer tier includes up to 100 live connections at no charge. Sandbox needs no certificate.

### Plaid

To use Plaid instead (or as well), set `PLAID_CLIENT_ID`, `PLAID_SECRET`, and optional `PLAID_ENV=sandbox` in `.env.local` or the container environment. Anchored only asks Plaid for read-only access: transactions, balances, investments, and optional liabilities. It does not request transfers, payments, Auth, or account numbers for moving money. The gate lives in `src/plaid/mod.rs`.

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
