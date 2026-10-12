import { NextRequest, NextResponse } from "next/server";
import { seedDatabase } from "@/lib/seed";

export const runtime = "nodejs";

export async function POST(req: NextRequest) {
  const body = await req.json().catch(() => ({}));
  const fresh = Boolean(body?.fresh);
  const result = seedDatabase(fresh);
  return NextResponse.json(result);
}
