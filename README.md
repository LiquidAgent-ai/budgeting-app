# Anchored for Windows

Anchored is a personal finance app that stays on your computer. This is the Windows app. It opens its own window and keeps your books in a SQLite file on that PC. You do not start a separate dev server to use the installed app.

You can also run Anchored in one container with Podman or Docker, including on TrueNAS. That is separate from the Windows app. The steps are in [Run it in a container](#run-it-in-a-container).

## Install

Download the installer and double-click it:

[Anchored-Setup-1.0.0.exe](https://github.com/LiquidAgent-ai/budgeting-app/releases/latest/download/Anchored-Setup-1.0.0.exe)

That file is a normal Windows setup program. It adds an **Anchored** shortcut to the Start menu and to the desktop. The installed app includes its own server and does not need Node.js.

GitHub builds this installer from `main` and attaches it to the [latest release](https://github.com/LiquidAgent-ai/budgeting-app/releases/latest).

To build the same Setup.exe yourself, use a Windows PC:

```bash
npm install
npm run desktop:dist
```

The file is written to `dist-desktop\Anchored-Setup-1.0.0.exe`.

On a Mac or Linux machine the same command prepares the server files and then stops. It cannot produce the Windows Setup.exe.

## Where your data lives

The installed app, and the desktop window you open while developing, store the database here:

- Windows: `%APPDATA%\Anchored\anchored.db`
- Linux: `~/.config/Anchored/anchored.db`
- Mac: `~/Library/Application Support/Anchored/anchored.db`

Updating or uninstalling the app leaves that file in place.

On a Mac or Linux PC, Anchored limits that database file, `plaid.env`, and the server log to your user account. On Windows, those files live in your user profile, which other accounts on the PC cannot read.

The database file is encrypted. A normal SQLite program cannot read it. Someone signed in as you can still open Anchored and see the books, because the app can unlock the file for that account. Copying the file to another person or another computer does not give them the books.

The first time you open the dashboard on a new install, Anchored loads a demo household so the screens are not empty. If the database file was already there when the app started, it is left alone, even if you deleted every account. An older file that is not encrypted yet is encrypted the first time this version opens it. The accounts and transactions stay, and the unprotected copy is removed.

`npm run dev` uses `data/anchored.db` inside this folder, unless you set `DATABASE_PATH` to another full path. The container uses `/data/anchored.db` unless you set `DATABASE_PATH`. The Windows app sets the path to the file above.

If the window says Anchored could not start, look at the log in [Logs](#logs).

## The database key

Anchored will not create or open the database when the key is missing or wrong, and it will not replace a file it cannot unlock.

**Windows app.** The first time Anchored starts, it makes a random key and stores it with Windows for your account (the OS credential store). You do not choose the key, and it is not written down as text. The sealed copy is `%APPDATA%\Anchored\db-key.bin`. The next time you sign in as the same user on the same PC, Anchored unlocks the database by itself.

There is no password to look up or reset. If you delete `db-key.bin`, or you copy only `anchored.db` to another PC or another Windows account, the books cannot be opened. The sealed key works for this Windows user on this PC. It does not work anywhere else.

**This folder, or a container.** You supply the key. Put a long random string in `.env.local` as `DATABASE_KEY` (copy `.env.example` and replace `your_database_key`). Keep that same string somewhere safe, such as a password manager. For a container, pass the same value every time the container starts. Do not put the key in the image. The commands are in [Run it in a container](#run-it-in-a-container).

If `DATABASE_KEY` is missing or different from the one that encrypted the file, Anchored refuses to open it. Anchored does not keep a second copy. If you lose the string, the data cannot be recovered.

## Run it in a container

The container is one way to run Anchored on TrueNAS, Podman, or Docker. Your books stay in a database file on a disk you choose. The file is encrypted with `DATABASE_KEY`.

`npm run dev` and `npm start` stay on this computer. They listen on 127.0.0.1 only. The container does not change that.

Inside the container, Anchored listens on port 3000 so the published port works. Nginx Proxy Manager reaches Anchored through that published port. Do not point the proxy at the dev server, and do not turn on host networking.

Make a long random string and keep it. You will pass it as `DATABASE_KEY` every time. If it is missing, still the example placeholder, or shorter than 16 characters, the container will not start.

### Podman

Build the image, then start it. Replace the dataset path with yours, and replace the key:

```bash
podman build -t anchored .

podman run -d --name anchored \
  --restart unless-stopped \
  -p 3000:3000 \
  -v /mnt/tank/anchored:/data \
  -e DATABASE_KEY='paste-a-long-random-string' \
  -e PUID=568 \
  -e PGID=568 \
  anchored
```

### Docker

```bash
docker build -t anchored .

docker run -d --name anchored \
  --restart unless-stopped \
  -p 3000:3000 \
  -v /mnt/tank/anchored:/data \
  -e DATABASE_KEY='paste-a-long-random-string' \
  -e PUID=568 \
  -e PGID=568 \
  anchored
```

### TrueNAS dataset

Create a dataset for the books, for example `/mnt/tank/anchored`. Mount that dataset at `/data` in the container. Anchored stores the encrypted file at `/data/anchored.db`.

The container writes that file as user 568, group 568. That is the TrueNAS apps user. `PUID` and `PGID` default to 568 when you leave them out. `/data` has to be writable by that user.

### Compose

From this folder, create a file named `.env`. Compose reads it. Do not commit it.

```text
DATABASE_KEY=paste-a-long-random-string
ANCHORED_DATA=/mnt/tank/anchored
```

`ANCHORED_DATA` is the dataset path on the machine. If you leave it out, Compose uses a named volume instead. Set `PORT=8080` in that same file if port 3000 is already taken on the host. The app inside the container still uses port 3000. The host port is the one you give to the proxy.

Optional, when you browse Anchored at a public https address:

```text
APP_URL=https://budget.example.com
```

Anchored uses `APP_URL` when it needs a full link. With that set, browser calls have to come from that same address. It also honors `X-Forwarded-Proto` and `X-Forwarded-Host` from the proxy.

Start it:

```bash
podman compose up -d --build
```

or:

```bash
docker compose up -d --build
```

Open `http://127.0.0.1:3000` on that machine, or the host port you set. A check of the server is `GET /api/health`.

### Nginx Proxy Manager

Add a proxy host that points at the published port:

- Scheme: `http`
- Forward hostname: the machine where the container is running
- Forward port: `3000`, or the host `PORT` you published
- Websockets: on
- SSL: request a certificate on the proxy host

Anchored itself stays on http. The proxy is what browsers use with https. The published port is how Nginx Proxy Manager reaches Anchored. You do not need to open the dev server to your network, and you do not need host networking. Do not forward port 3000 from your router to the internet. People should come in through the proxy.

## Browser, while you are developing

```bash
npm install
cp .env.example .env.local
```

Edit `.env.local` and set `DATABASE_KEY` to a long random string. Then:

```bash
npm run dev
```

Open http://127.0.0.1:3000. `npm run dev` and `npm start` listen on 127.0.0.1 only, so other computers on the network cannot open the app. `localhost` still works when it points at that address. The container is the run that publishes a port. It does not change these two commands.

```bash
npm run seed       # demo household; does not replace accounts already stored
npm run db:reset   # delete the database and load the demo household
```

## Desktop window, while you are developing

```bash
npm run desktop:dev
```

That opens an Anchored window and starts the app for you. Quit the window and the server stops with it.

## Bank linking

The installer contains no bank keys and no `.env` file.

To link a bank from the Windows app, create `%APPDATA%\Anchored\plaid.env`:

```text
PLAID_CLIENT_ID=your_client_id
PLAID_SECRET=your_sandbox_secret
PLAID_ENV=sandbox
```

Restart Anchored. For `npm run dev`, copy `.env.example` to `.env.local` and use the same kind of values. Do not commit that file.

Anchored only asks Plaid for read-only access: transactions, balances, investments, and optional liabilities. It does not request transfers, payments, or account numbers for moving money.

## Logs

Anchored writes one plain line when something actually happens: the server starts, the database opens or closes, a bank is linked, a sync finishes or fails, or a request fails. Successful pages, static files, and `/api/health` checks are left out. The database key, bank tokens, account numbers, and transaction details are not written.

A container shows those lines in `podman logs anchored` or `docker logs anchored`. The Windows app writes them to `%APPDATA%\Anchored\server.log`, and keeps the previous file when that log passes 1 MB. `npm run dev` prints them in the terminal.

## Commands

| Command | What it does |
| --- | --- |
| `npm run dev` | Website at http://localhost:3000 |
| `npm run desktop:dev` | Anchored window for development |
| `npm run desktop:dist` | Build the Windows Setup.exe (on Windows) |
| `npm run build` | Production website build |
| `npm start` | Serve that production build |
| `npm run seed` | Add the demo household if there are no accounts |
| `npm run db:reset` | Delete the database and load the demo household |
