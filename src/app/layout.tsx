import type { Metadata } from "next";
import { Fraunces, Outfit } from "next/font/google";
import { headers } from "next/headers";
import { publicOrigin } from "@/lib/public-url";
import "./globals.css";

const fraunces = Fraunces({
  subsets: ["latin"],
  variable: "--font-fraunces",
  display: "swap",
});

const outfit = Outfit({
  subsets: ["latin"],
  variable: "--font-outfit",
  display: "swap",
});

export async function generateMetadata(): Promise<Metadata> {
  const headerList = await headers();
  const origin = publicOrigin({
    host: headerList.get("host"),
    forwardedHost: headerList.get("x-forwarded-host"),
    forwardedProto: headerList.get("x-forwarded-proto"),
  });
  return {
    ...(origin ? { metadataBase: new URL(origin) } : {}),
    title: "Anchored — your money, locally hosted",
    description:
      "Link checking, credit, and investment accounts. Track spending, budgets, subscriptions, and net worth — on your machine.",
  };
}

export default function RootLayout({ children }: { children: React.ReactNode }) {
  return (
    <html lang="en">
      <body className={`${fraunces.variable} ${outfit.variable} antialiased`}>{children}</body>
    </html>
  );
}
