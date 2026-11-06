import { NextResponse } from "next/server";
import type { NextRequest } from "next/server";
import { requestIsBlocked } from "@/lib/local-access";

export function middleware(request: NextRequest) {
  if (
    requestIsBlocked({
      host: request.headers.get("host"),
      origin: request.headers.get("origin"),
      site: request.headers.get("sec-fetch-site"),
      pathname: request.nextUrl.pathname,
    })
  ) {
    return new NextResponse("Forbidden", {
      status: 403,
      headers: { "Cache-Control": "no-store" },
    });
  }

  const response = NextResponse.next();
  response.headers.set("X-Content-Type-Options", "nosniff");
  response.headers.set("X-Frame-Options", "DENY");
  response.headers.set("Referrer-Policy", "no-referrer");
  response.headers.set("Content-Security-Policy", "frame-ancestors 'none'");
  if (request.nextUrl.pathname.startsWith("/api")) {
    response.headers.set("Cache-Control", "no-store");
  }
  return response;
}

export const config = {
  matcher: ["/((?!_next/static|_next/webpack|favicon.ico).*)"],
};
