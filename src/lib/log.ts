import { isContainerServer } from "@/lib/public-url";
import { redactSecrets, safeErrorName } from "@/lib/safe-error";

/**
 * One plain line per real event, written to stdout (info) or stderr (error).
 * The terminal shows it for npm run dev. Electron appends it to server.log.
 * The container shows it in podman logs / docker logs.
 * Successful requests, health checks, SQL, and secrets are not logged here.
 */

type Level = "info" | "error";

type StartState = {
  attempted?: boolean;
  started?: boolean;
  listen?: string;
  announced?: string;
  corrected?: boolean;
  database?: "open" | "failed";
  migrated?: boolean;
};

const STATUS_NAMES: Record<number, string> = {
  400: "BadRequest",
  401: "Unauthorized",
  403: "Forbidden",
  404: "NotFound",
  405: "MethodNotAllowed",
  408: "Timeout",
  409: "Conflict",
  413: "PayloadTooLarge",
  422: "Unprocessable",
  429: "TooManyRequests",
  500: "InternalError",
  502: "BadGateway",
  503: "Unavailable",
  504: "Timeout",
};

function startState(): StartState {
  const g = globalThis as typeof globalThis & { __anchoredLogStart?: StartState };
  if (!g.__anchoredLogStart) g.__anchoredLogStart = {};
  return g.__anchoredLogStart;
}

export function runtimeMode(): "dev" | "desktop" | "container" {
  if (isContainerServer()) return "container";
  const desktop = (process.env.ANCHORED_DESKTOP ?? "").trim().toLowerCase();
  if (desktop === "1" || desktop === "true" || desktop === "yes") return "desktop";
  return "dev";
}

function cleanListen(address: string): string | null {
  const trimmed = address.trim();
  if (!trimmed || trimmed.length > 80) return null;
  if (!/^[A-Za-z0-9.:[\]-]+$/.test(trimmed)) return null;
  return trimmed;
}

export function expectedListenAddress(): string {
  const raw = process.env.PORT?.trim() || "3000";
  const port = /^[0-9]{1,5}$/.test(raw) ? Number(raw) : 3000;
  const safePort = port >= 1 && port <= 65535 ? String(port) : "3000";
  const host = isContainerServer() ? "0.0.0.0" : "127.0.0.1";
  return `${host}:${safePort}`;
}

export function formatBoundAddress(address: unknown): string | null {
  if (typeof address === "string") return cleanListen(address);
  if (!address || typeof address !== "object") return null;
  const info = address as { address?: unknown; port?: unknown };
  if (typeof info.port !== "number" || !Number.isFinite(info.port)) return null;
  const host = typeof info.address === "string" && info.address ? info.address : "127.0.0.1";
  const rendered = host.includes(":") && !host.startsWith("[") ? `[${host}]` : host;
  return cleanListen(`${rendered}:${info.port}`);
}

function oneLine(text: string): string {
  return redactSecrets(text).replace(/[\r\n\t]+/g, " ").trim().slice(0, 360);
}

function isProduction(): boolean {
  return process.env.NODE_ENV === "production";
}

function shortStack(err: unknown): string {
  if (isProduction() || !(err instanceof Error) || !err.stack) return "";
  const frames: string[] = [];
  for (const line of err.stack.split("\n")) {
    const frame = parseFrame(line);
    if (!frame) continue;
    frames.push(frame);
    if (frames.length >= 3) break;
  }
  if (!frames.length) return "";
  const text = oneLine(frames.join(" <- "));
  if (!text || text.includes("[redacted]")) return "";
  return ` at ${text}`;
}

function parseFrame(line: string): string | null {
  const trimmed = line.trim();
  if (!trimmed.startsWith("at ")) return null;
  let body = trimmed.slice(3).trim();
  if (body.startsWith("async ")) body = body.slice(6).trim();
  let fn = "";
  const paren = body.lastIndexOf("(");
  if (paren !== -1 && body.endsWith(")")) {
    fn = body.slice(0, paren).trim();
    body = body.slice(paren + 1, -1);
  }
  const loc = body.match(/:(\d+):\d+$/);
  if (!loc) return null;
  const file = body.slice(0, body.length - loc[0].length);
  if (file.includes("node_modules") || file.startsWith("node:")) return null;
  const base = file.split(/[/\\]/).pop() || "";
  if (!base || base === "log.ts") return null;
  const label = `${fn ? `${fn} ` : ""}${base}:${loc[1]}`.trim();
  if (!label || label.length > 120) return null;
  return label;
}

function writeLine(level: Level, message: string, err?: unknown): void {
  const text = oneLine(message);
  if (!text) return;
  const stack = level === "error" ? shortStack(err) : "";
  const line = `${new Date().toISOString()} ${level} ${text}${stack}\n`;
  const fd = level === "error" ? 2 : 1;
  try {
    // Instrumentation is also compiled for the edge runtime, which cannot import node builtins.
    // The lookup stays inside the function so the edge bundle never resolves "fs".
    const fs = process.getBuiltinModule("fs") as { writeSync: (fd: number, data: string) => number };
    fs.writeSync(fd, line);
  } catch {
    try {
      (fd === 2 ? process.stderr : process.stdout).write(line);
    } catch {
      // A closed pipe must not take down a request or shutdown.
    }
  }
}

export function logInfo(message: string): void {
  writeLine("info", message);
}

export function logError(message: string, err?: unknown): void {
  const reason = err ? safeErrorName(err, "Error") : "";
  const text = reason && !message.includes(reason) ? `${message}: ${reason}` : message;
  writeLine("error", text, err);
}

function asCount(value: number): string {
  if (!Number.isFinite(value) || value < 0) return "0";
  const whole = Math.trunc(value);
  if (whole > 1_000_000_000) return "1000000000";
  return String(whole);
}

export function logPlaidLinkStarted(): void {
  logInfo("plaid link started");
}

export function logPlaidLinkFailed(err: unknown): void {
  logError("plaid link failed", err);
}

export function logPlaidItemConnected(accounts: number): void {
  logInfo(`plaid item connected accounts=${asCount(accounts)}`);
}

export function logPlaidItemConnectFailed(err: unknown): void {
  logError("plaid item connect failed", err);
}

export function logPlaidSyncStarted(): void {
  logInfo("plaid sync started");
}

export function logPlaidSyncFinished(accounts: number, transactions: number): void {
  logInfo(`plaid sync finished accounts=${asCount(accounts)} transactions=${asCount(transactions)}`);
}

export function logPlaidSyncFailed(err: unknown): void {
  logError("plaid sync failed", err);
}

function maybeLogStart(): void {
  const state = startState();
  if (state.started || !state.listen || !state.database) return;
  state.started = true;
  state.announced = state.listen;
  logInfo(
    `server started mode=${runtimeMode()} listen=${state.listen} database=${state.database} migrated=${state.migrated ? "yes" : "no"}`
  );
}

/** First database open in this process. Later opens do not change the startup line. */
export function noteDatabaseResult(opened: boolean, migrated: boolean): void {
  const state = startState();
  if (state.database) return;
  state.database = opened ? "open" : "failed";
  state.migrated = migrated;
  maybeLogStart();
}

/**
 * `bound` is the address the HTTP server actually accepted.
 * The expected address is only used until a bound address is known.
 */
export function noteListenAddress(address: string, bound: boolean): void {
  const clean = cleanListen(address);
  if (!clean) return;
  const state = startState();
  if (bound || !state.listen) state.listen = clean;
  maybeLogStart();
  if (!bound || !state.started || !state.announced || state.announced === clean || state.corrected) return;
  state.corrected = true;
  logInfo(`server listening on ${clean}`);
}

function requestPath(url: string): string | null {
  const raw = url || "/";
  const end = raw.indexOf("?");
  const path = (end === -1 ? raw : raw.slice(0, end)).replace(/[\r\n\t]/g, "");
  if (!path.startsWith("/")) return null;
  return path.slice(0, 200);
}

function skipRequestPath(path: string): boolean {
  if (path === "/api/health") return true;
  if (path.startsWith("/_next/") || path.startsWith("/__next")) return true;
  if (path === "/favicon.ico") return true;
  return /\.(?:js|mjs|cjs|css|map|png|jpe?g|gif|webp|svg|ico|woff2?|ttf|txt)$/i.test(path);
}

/** One line for a failed request. Successes, assets, hot reload, and health checks are skipped. */
export function logRequestFailure(method: string, url: string, status: number): void {
  try {
    if (!Number.isInteger(status) || status < 400 || status > 599) return;
    const path = requestPath(url);
    if (!path || skipRequestPath(path)) return;
    const verb = (method || "GET").toUpperCase().replace(/[^A-Z]/g, "").slice(0, 10) || "GET";
    const name = STATUS_NAMES[status] ?? `Http${status}`;
    const line = `${verb} ${path} ${status} ${name}`;
    if (status >= 500) logError(line);
    else logInfo(line);
  } catch {
    // logging must not change the response
  }
}

/** Open the database once and log how this process came up. Safe to call again after a reload. */
export async function logProcessStart(): Promise<void> {
  if (process.env.NEXT_PHASE === "phase-production-build") return;
  const state = startState();
  if (state.attempted) {
    noteListenAddress(expectedListenAddress(), false);
    return;
  }
  state.attempted = true;
  try {
    const db = await import("./db");
    db.getDb();
  } catch (err) {
    let migrated = false;
    try {
      const db = await import("./db");
      migrated = db.startupMigrated();
    } catch {
      migrated = false;
    }
    noteDatabaseResult(false, migrated);
    logError("database open failed", err);
  } finally {
    noteListenAddress(expectedListenAddress(), false);
  }
}
