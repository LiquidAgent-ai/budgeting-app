import type { NextConfig } from "next";

const nextConfig: NextConfig = {
  // Packaged Windows app runs this output with its own Node, not `next dev`.
  output: "standalone",
  serverExternalPackages: ["better-sqlite3-multiple-ciphers"],
  outputFileTracingExcludes: {
    "*": [".env", ".env.*", "data/**/*"],
  },
  webpack: (config, { nextRuntime }) => {
    // Middleware causes an edge build of instrumentation. That bundle never runs
    // the database startup, but webpack still follows its imports.
    if (nextRuntime === "edge") {
      config.resolve.alias = {
        ...config.resolve.alias,
        fs: false,
        path: false,
        "better-sqlite3-multiple-ciphers": false,
      };
    }
    return config;
  },
};

export default nextConfig;
