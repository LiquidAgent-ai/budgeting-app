"use strict";

const { app, BrowserWindow, dialog, Menu, safeStorage, session, shell } = require("electron");
const { spawn } = require("child_process");
const crypto = require("crypto");
const fs = require("fs");
const http = require("http");
const net = require("net");
const os = require("os");
const path = require("path");

const PLAID_KEYS = ["PLAID_CLIENT_ID", "PLAID_SECRET", "PLAID_ENV"];
const LOG_SECRET_KEYS = ["PLAID_CLIENT_ID", "PLAID_SECRET", "DATABASE_KEY"];

if (process.platform !== "win32") {
  process.umask(0o077);
}

let mainWindow = null;
let serverProcess = null;
let serverLog = null;
let databaseKey = null;

function dataRoot() {
  if (process.platform === "win32") {
    const base = process.env.APPDATA || path.join(os.homedir(), "AppData", "Roaming");
    return path.join(base, "Anchored");
  }
  if (process.platform === "darwin") {
    return path.join(os.homedir(), "Library", "Application Support", "Anchored");
  }
  const base = process.env.XDG_CONFIG_HOME || path.join(os.homedir(), ".config");
  return path.join(base, "Anchored");
}

app.setName("Anchored");
app.setPath("userData", dataRoot());
app.setAppUserModelId("app.anchored.desktop");

function databasePath() {
  return path.join(app.getPath("userData"), "anchored.db");
}

function databaseKeyPath() {
  return path.join(app.getPath("userData"), "db-key.bin");
}

// The Windows app creates a random key and seals it with the OS credential
// store (DPAPI on Windows). The user does not choose or type it. The sealed
// blob is useless on another account or another PC.
function loadOrCreateDatabaseKey() {
  if (!safeStorage.isEncryptionAvailable()) {
    throw new Error(
      "Anchored cannot unlock its database because this computer's credential store is unavailable. The database was not opened."
    );
  }
  const file = databaseKeyPath();
  restrictDataRoot(path.dirname(file));
  if (fs.existsSync(file)) {
    let key = "";
    try {
      key = safeStorage.decryptString(fs.readFileSync(file));
    } catch {
      throw new Error(
        "Anchored could not read the database key from the OS credential store. The database was not opened."
      );
    }
    if (!key || key.trim().length < 16) {
      throw new Error(
        "Anchored could not read the database key from the OS credential store. The database was not opened."
      );
    }
    restrictFile(file);
    return key;
  }
  const key = crypto.randomBytes(32).toString("base64url");
  const wrapped = safeStorage.encryptString(key);
  const tmp = `${file}.tmp`;
  fs.writeFileSync(tmp, wrapped, { mode: 0o600 });
  restrictFile(tmp);
  fs.renameSync(tmp, file);
  restrictFile(file);
  return key;
}

function restrictFile(file) {
  try {
    if (fs.existsSync(file)) fs.chmodSync(file, 0o600);
  } catch {
    // Windows profile ACLs, or a file whose mode cannot be changed.
  }
}

function restrictDataRoot(dir) {
  fs.mkdirSync(dir, { recursive: true });
  if (process.platform === "win32") return;
  if (path.basename(dir) !== "Anchored") return;
  if (dir === path.parse(dir).root) return;
  try {
    fs.chmodSync(dir, 0o700);
  } catch {
    // The directory is still usable.
  }
}

function redactOutput(text, env) {
  let out = String(text);
  for (const key of LOG_SECRET_KEYS) {
    const value = env && env[key];
    if (typeof value === "string" && value.length >= 8) out = out.split(value).join("[redacted]");
  }
  return out
    .replace(/access-(?:sandbox|development|production)-[A-Za-z0-9_-]+/g, "[redacted]")
    .replace(/public-(?:sandbox|development|production)-[A-Za-z0-9_-]+/g, "[redacted]")
    .replace(/link-(?:sandbox|development|production)-[A-Za-z0-9_-]+/g, "[redacted]");
}

function readPlaidEnv(file) {
  if (!fs.existsSync(file)) return {};
  restrictFile(file);
  const allowed = new Set(PLAID_KEYS);
  const out = {};
  const text = fs.readFileSync(file, "utf8");
  for (const line of text.split(/\r?\n/)) {
    const trimmed = line.trim();
    if (!trimmed || trimmed.startsWith("#")) continue;
    const eq = trimmed.indexOf("=");
    if (eq <= 0) continue;
    const key = trimmed.slice(0, eq).trim();
    if (!allowed.has(key)) continue;
    let value = trimmed.slice(eq + 1).trim();
    if (
      (value.startsWith('"') && value.endsWith('"')) ||
      (value.startsWith("'") && value.endsWith("'"))
    ) {
      value = value.slice(1, -1);
    }
    if (value) out[key] = value;
  }
  return out;
}

function getFreePort() {
  return new Promise((resolve, reject) => {
    const server = net.createServer();
    const fail = (err) => {
      server.close(() => reject(err));
    };
    server.once("error", fail);
    server.listen(0, "127.0.0.1", () => {
      const address = server.address();
      const port = typeof address === "object" && address ? address.port : 0;
      server.close((err) => (err ? reject(err) : resolve(port)));
    });
  });
}

function waitForHttp(port, timeoutMs) {
  const started = Date.now();
  return new Promise((resolve, reject) => {
    let settled = false;
    let timer = null;
    let active = null;
    const finish = (err) => {
      if (settled) return;
      settled = true;
      if (timer) clearTimeout(timer);
      if (active) {
        active.destroy();
        active = null;
      }
      if (err) reject(err);
      else resolve();
    };
    const schedule = () => {
      if (settled || timer) return;
      if (Date.now() - started > timeoutMs) {
        finish(new Error("Anchored took too long to start."));
        return;
      }
      timer = setTimeout(() => {
        timer = null;
        attempt();
      }, 300);
    };
    const attempt = () => {
      if (settled) return;
      if (serverProcess && serverProcess.spawnError) {
        finish(serverProcess.spawnError);
        return;
      }
      if (serverProcess && serverProcess.exitCode != null) {
        finish(new Error("Anchored server stopped before the window could open."));
        return;
      }
      const req = http.get({ hostname: "127.0.0.1", port, path: "/", timeout: 2000 }, (res) => {
        res.resume();
        active = null;
        if (res.statusCode && res.statusCode < 500) finish();
        else schedule();
      });
      active = req;
      req.on("error", () => {
        if (active === req) active = null;
        schedule();
      });
      req.on("timeout", () => {
        req.destroy();
      });
    };
    attempt();
  });
}

function openLog() {
  const dir = app.getPath("userData");
  restrictDataRoot(dir);
  const logPath = path.join(dir, "server.log");
  try {
    if (fs.existsSync(logPath) && fs.statSync(logPath).size > 1024 * 1024) {
      const previous = logPath + ".1";
      fs.rmSync(previous, { force: true });
      fs.renameSync(logPath, previous);
      restrictFile(previous);
    }
  } catch {
    // Keep appending to the current log.
  }
  serverLog = fs.createWriteStream(logPath, { flags: "a" });
  serverLog.on("error", () => {});
  restrictFile(logPath);
  serverLog.write(`\n--- ${new Date().toISOString()} ---\n`);
}

function rememberOutput(child, env) {
  let tail = "";
  const take = (chunk) => {
    const text = redactOutput(chunk.toString(), env);
    tail = (tail + text).slice(-4000);
    child.outputTail = tail;
    if (serverLog) serverLog.write(text);
  };
  child.stdout?.on("data", take);
  child.stderr?.on("data", take);
}

function childEnv(port) {
  if (!databaseKey) {
    throw new Error("The database key is not available. Anchored did not start the server.");
  }
  const env = { ...process.env };
  delete env.ELECTRON_RUN_AS_NODE;
  env.DATABASE_PATH = databasePath();
  env.DATABASE_KEY = databaseKey;
  env.PORT = String(port);
  env.HOSTNAME = "127.0.0.1";
  env.HOST = "127.0.0.1";
  env.ANCHORED_DESKTOP = "1";
  const plaid = readPlaidEnv(path.join(app.getPath("userData"), "plaid.env"));
  for (const key of PLAID_KEYS) {
    if (plaid[key]) env[key] = plaid[key];
  }
  return env;
}

function spawnServer(command, args, env, cwd) {
  const child = spawn(command, args, {
    cwd,
    env,
    detached: process.platform !== "win32",
    windowsHide: true,
    stdio: ["ignore", "pipe", "pipe"],
  });
  child.on("error", (err) => {
    child.spawnError = err;
  });
  rememberOutput(child, env);
  return child;
}

function startDevServer(port) {
  const appPath = app.getAppPath();
  const nextBin = path.join(appPath, "node_modules", "next", "dist", "bin", "next");
  if (!fs.existsSync(nextBin)) {
    throw new Error("Next.js is not installed. From the project folder run npm install, then npm run desktop:dev.");
  }
  const nodeBin = process.platform === "win32" ? "node.exe" : "node";
  return spawnServer(
    nodeBin,
    [nextBin, "dev", "-H", "127.0.0.1", "-p", String(port)],
    childEnv(port),
    appPath
  );
}

function startPackagedServer(port) {
  const standaloneDir = path.join(process.resourcesPath, "standalone");
  const serverJs = path.join(standaloneDir, "server.js");
  const nodeBinary = path.join(standaloneDir, "node.exe");
  if (!fs.existsSync(serverJs) || !fs.existsSync(nodeBinary)) {
    throw new Error(
      "This install is missing its built-in server. Rebuild the Windows installer with npm run desktop:dist on Windows."
    );
  }
  const env = childEnv(port);
  env.NODE_ENV = "production";
  return spawnServer(nodeBinary, [serverJs], env, standaloneDir);
}

function killServer() {
  const child = serverProcess;
  serverProcess = null;
  if (!child || child.pid == null) return;
  if (process.platform === "win32") {
    spawn("taskkill", ["/pid", String(child.pid), "/t", "/f"], {
      windowsHide: true,
      stdio: "ignore",
    });
    return;
  }
  try {
    process.kill(-child.pid, "SIGTERM");
  } catch {
    try {
      child.kill("SIGTERM");
    } catch {
      // already gone
    }
  }
}

function isAppUrl(url, port) {
  try {
    const parsed = new URL(url);
    return (
      parsed.protocol === "http:" &&
      parsed.hostname === "127.0.0.1" &&
      parsed.port === String(port) &&
      !parsed.username &&
      !parsed.password
    );
  } catch {
    return false;
  }
}

function isPlaidHttps(url) {
  try {
    const parsed = new URL(url);
    if (parsed.protocol !== "https:") return false;
    if (parsed.username || parsed.password) return false;
    if (parsed.port && parsed.port !== "443") return false;
    const host = parsed.hostname.toLowerCase();
    return host === "plaid.com" || host.endsWith(".plaid.com");
  } catch {
    return false;
  }
}

function isAllowedFrameUrl(url, isMainFrame, port) {
  if (!isMainFrame && url === "about:blank") return true;
  if (isMainFrame) return isAppUrl(url, port);
  return isAppUrl(url, port) || isPlaidHttps(url);
}

function navigationTarget(details, url, isMainFrame) {
  if (details && typeof details.url === "string") {
    return {
      event: details,
      url: details.url,
      isMainFrame: details.isMainFrame !== false,
    };
  }
  return {
    event: details,
    url,
    isMainFrame: isMainFrame !== false,
  };
}

function hardenContents(contents, port) {
  contents.setWindowOpenHandler(({ url }) => {
    // The OS browser only. A new Electron window would not keep the app sandbox.
    if (isPlaidHttps(url)) {
      shell.openExternal(url).catch(() => {});
    }
    return { action: "deny" };
  });

  const guard = (event, url, isMainFrame) => {
    const isAppWindow = mainWindow && contents === mainWindow.webContents;
    if (isAppWindow) {
      if (!isAllowedFrameUrl(url, isMainFrame, port)) event.preventDefault();
      return;
    }
    if (!isPlaidHttps(url) && url !== "about:blank") event.preventDefault();
  };

  const onNavigate = (details, url, _isInPlace, isMainFrame) => {
    const nav = navigationTarget(details, url, isMainFrame);
    guard(nav.event, nav.url, nav.isMainFrame);
  };

  contents.on("will-navigate", onNavigate);
  contents.on("will-redirect", onNavigate);
  contents.on("will-frame-navigate", onNavigate);
  contents.on("will-attach-webview", (event) => {
    event.preventDefault();
  });
}

function installContentGuards(port) {
  session.defaultSession.setPermissionRequestHandler((_contents, _permission, callback) => {
    callback(false);
  });
  session.defaultSession.setPermissionCheckHandler(() => false);
  app.on("web-contents-created", (_event, contents) => {
    hardenContents(contents, port);
  });
}

async function createWindow(port) {
  mainWindow = new BrowserWindow({
    width: 1280,
    height: 840,
    minWidth: 880,
    minHeight: 640,
    title: "Anchored",
    backgroundColor: "#fbfaf7",
    show: false,
    autoHideMenuBar: true,
    webPreferences: {
      preload: path.join(__dirname, "preload.cjs"),
      contextIsolation: true,
      nodeIntegration: false,
      nodeIntegrationInWorker: false,
      nodeIntegrationInSubFrames: false,
      sandbox: true,
      webSecurity: true,
      allowRunningInsecureContent: false,
      navigateOnDragDrop: false,
    },
  });

  mainWindow.once("ready-to-show", () => {
    if (mainWindow && !mainWindow.isDestroyed()) mainWindow.show();
  });

  const loading =
    "data:text/html;charset=utf-8," +
    encodeURIComponent(
      "<!doctype html><title>Anchored</title><body style=\"margin:0;display:grid;place-items:center;height:100vh;background:#fbfaf7;color:#0c2f2a;font-family:Georgia,serif\"><p>Starting Anchored…</p></body>"
    );
  await mainWindow.loadURL(loading);
  if (!mainWindow.isVisible()) mainWindow.show();
  await mainWindow.loadURL(`http://127.0.0.1:${port}/`);
}

async function start() {
  Menu.setApplicationMenu(null);
  openLog();
  restrictDataRoot(path.dirname(databasePath()));

  try {
    databaseKey = loadOrCreateDatabaseKey();
  } catch (err) {
    dialog.showErrorBox(
      "Anchored could not start",
      err instanceof Error ? err.message : "The database key could not be opened."
    );
    app.quit();
    return;
  }

  const port = await getFreePort();
  installContentGuards(port);
  try {
    serverProcess = app.isPackaged ? startPackagedServer(port) : startDevServer(port);
  } catch (err) {
    dialog.showErrorBox("Anchored could not start", err instanceof Error ? err.message : String(err));
    app.quit();
    return;
  }

  const timeout = app.isPackaged ? 60000 : 120000;
  try {
    await waitForHttp(port, timeout);
    if (serverLog) serverLog.write(`Anchored listening on 127.0.0.1:${port}\n`);
    await createWindow(port);
  } catch (err) {
    const detail = serverProcess?.outputTail ? `\n\n${serverProcess.outputTail}` : "";
    const message = (err instanceof Error ? err.message : "Anchored could not start") + detail;
    dialog.showErrorBox("Anchored could not start", message.slice(0, 1800));
    app.quit();
  }
}

const gotLock = app.requestSingleInstanceLock();
if (!gotLock) {
  app.quit();
} else {
  app.on("second-instance", () => {
    if (!mainWindow || mainWindow.isDestroyed()) return;
    if (mainWindow.isMinimized()) mainWindow.restore();
    mainWindow.focus();
  });
  app.on("before-quit", () => {
    killServer();
    if (serverLog) serverLog.end();
  });
  app.on("window-all-closed", () => {
    app.quit();
  });
  app.whenReady().then(start).catch((err) => {
    dialog.showErrorBox("Anchored could not start", err instanceof Error ? err.message : String(err));
    app.quit();
  });
}
