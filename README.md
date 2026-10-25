# Anchored for Windows

Anchored is a personal finance app that stays on your computer. This is the Windows app. It opens its own window and keeps your books in a SQLite file on that PC. You do not start a separate dev server to use the installed app.

A TrueNAS or other container install would be a different way to run Anchored. It is not this Windows app, and those container files are not part of this project.

## Install

Build the installer on a Windows PC:

```bash
npm install
npm run desktop:dist
```

The Setup file is `dist-desktop\Anchored-Setup-1.0.0.exe`. Run it and follow the steps. The installed app includes its own server and does not need Node.js. The command does not publish the file anywhere. There is no download page in this project: the Setup.exe only exists after that Windows build.

On a Mac or Linux machine the same command prepares the server files and then stops. It cannot produce the Windows Setup.exe.

## Where your data lives

The installed app, and the desktop window you open while developing, store the database here:

- Windows: `%APPDATA%\Anchored\anchored.db`
- Linux: `~/.config/Anchored/anchored.db`
- Mac: `~/Library/Application Support/Anchored/anchored.db`

Updating or uninstalling the app leaves that file in place.

The first time you open the dashboard on a new install, Anchored loads a demo household so the screens are not empty. If the database file was already there when the app started, it is left alone, even if you deleted every account.

`npm run dev` uses `data/anchored.db` inside this folder, unless you set `DATABASE_PATH` to another full path. The server and any future container keep using `DATABASE_PATH` the same way. The Windows app sets it to the file above.

If the window says Anchored could not start, look at `%APPDATA%\Anchored\server.log`.

## Browser, while you are developing

```bash
npm install
npm run dev
```

Open http://localhost:3000.

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
