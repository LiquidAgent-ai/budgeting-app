import { NextResponse } from "next/server";
import { getDashboardSummary } from "@/lib/finance";
import { seedDatabase } from "@/lib/seed";
import { databaseExistedAtStartup } from "@/lib/db";

export const runtime = "nodejs";

export async function GET() {
  // Demo data is only when this process started with no database file.
  // A file already on disk (an update, or a database you emptied) is not seeded.
  if (process.env.NEXT_PHASE !== "phase-production-build" && !databaseExistedAtStartup()) {
    seedDatabase(false);
  }
  return NextResponse.json(getDashboardSummary());
}
