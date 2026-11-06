/**
 * Decides whether an inbound HTTP request is allowed to touch this local app.
 * The server is for the person at this computer, not for other machines and
 * not for other websites the browser happens to have open.
 */

export function isLoopbackAddress(address: string | undefined | null): boolean {
  if (!address) return false;
  const bare = address.startsWith("::ffff:") ? address.slice("::ffff:".length) : address;
  if (bare === "::1") return true;
  const parts = bare.split(".");
  if (parts.length !== 4) return false;
  if (parts[0] !== "127") return false;
  return parts.every((part) => {
    if (!/^\d{1,3}$/.test(part)) return false;
    const n = Number(part);
    return n >= 0 && n <= 255;
  });
}

export function isLoopbackHost(hostHeader: string | null | undefined): boolean {
  if (!hostHeader) return false;
  const host = hostWithoutPort(hostHeader.trim().toLowerCase());
  if (host === "localhost" || host === "::1") return true;
  return isLoopbackAddress(host);
}

function hostWithoutPort(host: string): string {
  if (host.startsWith("[")) {
    const end = host.indexOf("]");
    return end === -1 ? host : host.slice(1, end);
  }
  const colon = host.lastIndexOf(":");
  if (colon > -1 && host.indexOf(":") === colon) return host.slice(0, colon);
  return host;
}

function canonicalHost(host: string, protocol: string): string {
  const lower = host.trim().toLowerCase();
  if (protocol === "http:" && lower.endsWith(":80")) return lower.slice(0, -3);
  if (protocol === "https:" && lower.endsWith(":443")) return lower.slice(0, -4);
  return lower;
}

export function originMatchesHost(origin: string, hostHeader: string): boolean {
  let url: URL;
  try {
    url = new URL(origin);
  } catch {
    return false;
  }
  if (url.protocol !== "http:" && url.protocol !== "https:") return false;
  return canonicalHost(url.host, url.protocol) === canonicalHost(hostHeader, url.protocol);
}

export function requestIsBlocked(input: {
  host: string | null;
  origin: string | null;
  site: string | null;
  pathname: string;
}): boolean {
  if (!isLoopbackHost(input.host)) return true;

  const path = input.pathname || "/";
  const guarded =
    path === "/api" || path.startsWith("/api/") || path === "/_next/image" || path.startsWith("/_next/image/");
  if (!guarded) return false;

  // Browsers set this. same-site still means another port on this machine.
  if (input.site && input.site !== "same-origin" && input.site !== "none") return true;
  if (input.origin && input.host && !originMatchesHost(input.origin, input.host)) return true;
  return false;
}
