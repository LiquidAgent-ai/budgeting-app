"use strict";

const { app, BrowserWindow, dialog, Menu } = require("electron");
const { spawn } = require("child_process");
const fs = require("fs");
const http = require("http");
const net = require("net");
const os = require("os");
const path = require("path");

const PLAID_KEYS = ["PLAID_CLIENT_ID", "PLAID_SECRET", "PLAID_ENV"];

let mainWindow = null;
let serverProcess = null;
let serverLog = null;

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

function readPlaidEnv(file) {
  if (!fs.existsSync(file)) return {};
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
    server.once("error", reject);
    server.listen(0, "127.0.0.1", () => {
      const address = server.address();
      const port = typeof address === "object" && address ? address.port : 0;
      server.close(() => resolve(port));
    });
  });
}

function waitForHttp(port, timeoutMs) {
  const started = Date.now();
  return new Promise((resolve, reject) => {
    const attempt = () => {
      if (serverProcess && serverProcess.spawnError) {
        reject(serverProcess.spawnError);
        return;
      }
      if (serverProcess && serverProcess.exitCode != null) {
        reject(new Error("Anchored server stopped before the window could open."));
        return;
      }
      const req = http.get({ hostname: "127.0.0.1", port, path: "/", timeout: 2000 }, (res) => {
        res.resume();
        if (res.statusCode && res.statusCode < 500) {
          resolve();
          return;
        }
        retry();
      });
      req.on("error", retry);
      req.on("timeout", () => {
        req.destroy();
        retry();
      });
    };
    const retry = () => {
      if (Date.now() - started > timeoutMs) {
        reject(new Error("Anchored took too long to start."));
        return;
      }
      setTimeout(attempt, 300);
    };
    attempt();
  });
}

function openLog() {
  fs.mkdirSync(app.getPath("userData"), { recursive: true });
  serverLog = fs.createWriteStream(path.join(app.getPath("userData"), "server.log"), { flags: "a" });
  serverLog.write(`\n--- ${new Date().toISOString()} ---\n`);
}

function rememberOutput(child) {
  let tail = "";
  const take = (chunk) => {
    const text = chunk.toString();
    tail = (tail + text).slice(-4000);
    child.outputTail = tail;
    if (serverLog) serverLog.write(text);
  };
  child.stdout?.on("data", take);
  child.stderr?.on("data", take);
}

function childEnv(port) {
  const env = { ...process.env };
  delete env.ELECTRON_RUN_AS_NODE;
  env.DATABASE_PATH = databasePath();
  env.PORT = String(port);
  env.HOSTNAME = "127.0.0.1";
  env.HOST = "127.0.0.1";
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
  rememberOutput(child);
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

function isAllowedUrl(url) {
  try {
    const parsed = new URL(url);
    if (parsed.protocol === "http:" && (parsed.hostname === "127.0.0.1" || parsed.hostname === "localhost")) {
      return true;
    }
    if (parsed.protocol !== "https:") return false;
    return parsed.hostname === "plaid.com" || parsed.hostname.endsWith(".plaid.com");
  } catch {
    return false;
  }
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
      sandbox: true,
    },
  });

  mainWindow.once("ready-to-show", () => {
    if (mainWindow && !mainWindow.isDestroyed()) mainWindow.show();
  });

  mainWindow.webContents.setWindowOpenHandler(({ url }) => {
    if (isAllowedUrl(url)) return { action: "allow" };
    return { action: "deny" };
  });

  mainWindow.webContents.on("will-navigate", (event, url) => {
    if (!isAllowedUrl(url)) event.preventDefault();
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
  fs.mkdirSync(path.dirname(databasePath()), { recursive: true });

  const port = await getFreePort();
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
