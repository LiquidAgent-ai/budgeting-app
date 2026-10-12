# Anchored

Locally hosted personal finance — a Rocket Money–style app that keeps your data in SQLite on your machine.

## What you get

- **Link banks** via [Plaid](https://plaid.com) (checking, credit cards, investments)
- **Manual accounts** when you do not want a third-party aggregator
- **Transactions** with auto-categorization
- **Budgets** per category
- **Subscription** detection
- **Net worth** across cash, credit, and investments
- **Demo household** so the UI is useful before any keys are set

## Quick start

```bash
cd anchored
cp .env.example .env.local   # optional until you link real/sandbox banks
npm install
npm run seed                 # optional; dashboard also auto-seeds if empty
npm run dev
```

Open [http://localhost:3000](http://localhost:3000).

## Link credit cards & checking (Plaid)

1. Create a free account at [dashboard.plaid.com](https://dashboard.plaid.com/)
2. Copy **Sandbox** `client_id` and `secret` into `.env.local`
3. Restart `npm run dev`
4. Go to **Accounts → Link bank with Plaid**
5. In sandbox, use Plaid’s test credentials (e.g. user `user_good`, password `pass_good`)

Production / Development Plaid keys work the same way — set `PLAID_ENV=development` or `production`. Investment holdings sync when the Investments product is enabled on your Plaid account.

## Data location

All data lives in `data/anchored.db` (gitignored). Reset with:

```bash
npm run db:reset
```

## Scripts

| Command        | Purpose                    |
|----------------|----------------------------|
| `npm run dev`  | Local Next.js server       |
| `npm run build`| Production build           |
| `npm start`    | Serve production build     |
| `npm run seed` | Load demo accounts / txns  |
| `npm run db:reset` | Wipe DB and re-seed   |

## Stack

Next.js 15 · React 19 · SQLite (`better-sqlite3`) · Plaid · Recharts · Tailwind CSS 4
