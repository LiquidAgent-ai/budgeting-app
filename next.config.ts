import type { NextConfig } from "next";

const nextConfig: NextConfig = {
  // Packaged Windows app runs this output with its own Node, not `next dev`.
  output: "standalone",
  serverExternalPackages: ["better-sqlite3"],
  outputFileTracingExcludes: {
    "*": [".env", ".env.*", "data/**/*"],
  },
};

export default nextConfig;
