import { isContainerServer } from "@/lib/public-url";
import { isLoopbackAddress, requestIsBlocked } from "./local-access";

type NodeRequest = {
  url?: string;
  headers: Record<string, string | string[] | undefined>;
  socket?: { remoteAddress?: string | null };
};

type NodeResponse = {
  statusCode: number;
  setHeader: (name: string, value: string) => void;
  end: (body?: string) => void;
};

let installed = false;

const CONTAINER_MARK = "x-anchored-container";

function header(req: NodeRequest, name: string): string | null {
  const value = req.headers[name];
  if (Array.isArray(value)) return value[0] ?? null;
  return value ?? null;
}

function pathnameOf(req: NodeRequest): string {
  const raw = req.url || "/";
  const end = raw.indexOf("?");
  return end === -1 ? raw : raw.slice(0, end);
}

function blocked(req: NodeRequest): boolean {
  // The container accepts the published port and the proxy. The PC does not.
  if (!isContainerServer() && !isLoopbackAddress(req.socket?.remoteAddress)) return true;
  return requestIsBlocked({
    host: header(req, "host"),
    origin: header(req, "origin"),
    site: header(req, "sec-fetch-site"),
    pathname: pathnameOf(req),
    forwardedProto: header(req, "x-forwarded-proto"),
    container: isContainerServer(),
  });
}

/**
 * On a PC, rejects connections that are not from this machine, and rejects
 * browser requests to the API that come from another site.
 * The container image sets ANCHORED_CONTAINER and listens on 0.0.0.0 so Nginx
 * Proxy Manager can use the published port. The dev server does not set that,
 * and npm run dev still passes --hostname 127.0.0.1.
 * Next binds before this hook runs. The bind address is HOSTNAME. This hook
 * still refuses a later listen on every interface unless this is the container.
 */
export function installLoopbackOnlyServer(): void {
  if (installed) return;
  installed = true;

  if (process.platform !== "win32" && process.env.NEXT_PHASE !== "phase-production-build") {
    process.umask(0o077);
  }

  const net = process.getBuiltinModule("net");
  const originalListen = net.Server.prototype.listen;
  net.Server.prototype.listen = function (this: typeof net.Server.prototype, ...args: unknown[]) {
    const nextArgs = args.slice();
    if (!isContainerServer()) {
      const first = nextArgs[0];
      if (typeof first === "number") {
        const host = nextArgs[1];
        if (host == null || host === "0.0.0.0" || host === "::" || host === "[::]") {
          nextArgs[1] = "127.0.0.1";
        }
      } else if (first && typeof first === "object") {
        const options = first as { host?: string; hostname?: string; fd?: unknown; path?: unknown; handle?: unknown };
        const usesHandle = options.fd != null || options.path != null || options.handle != null;
        const host = options.host ?? options.hostname;
        if (!usesHandle && (host == null || host === "0.0.0.0" || host === "::" || host === "[::]")) {
          nextArgs[0] = { ...options, host: "127.0.0.1" };
        }
      }
    }
    return originalListen.apply(this, nextArgs as never);
  } as typeof net.Server.prototype.listen;

  const http = process.getBuiltinModule("http");
  const original = http.Server.prototype.emit;
  http.Server.prototype.emit = function (this: typeof http.Server.prototype, event: string, ...args: unknown[]) {
    if (event === "request" || event === "upgrade") {
      const req = args[0] as NodeRequest;
      if (req && blocked(req)) {
        if (event === "request") {
          const res = args[1] as NodeResponse;
          res.statusCode = 403;
          res.setHeader("Content-Type", "text/plain; charset=utf-8");
          res.setHeader("Cache-Control", "no-store");
          res.end("Forbidden");
        } else {
          const socket = args[1] as { destroy?: () => void };
          socket.destroy?.();
        }
        return true;
      }
      if (req) {
        // Middleware runs in a bundle that may not see runtime env. The mark
        // is set here, after the client header is removed, and only for the container.
        if (isContainerServer()) req.headers[CONTAINER_MARK] = "1";
        else delete req.headers[CONTAINER_MARK];
      }
    }
    return original.call(this, event, ...args);
  } as typeof http.Server.prototype.emit;
}
