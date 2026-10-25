import { spawnSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const stageRoot = path.join(root, "desktop-bundle");
const stage = path.join(stageRoot, "standalone");

function run(command, args, extraEnv = {}) {
  const result = spawnSync(command, args, {
    cwd: root,
    stdio: "inherit",
    env: { ...process.env, ...extraEnv },
  });
  if (result.error) {
    console.error(result.error.message);
    process.exit(1);
  }
  if (result.status !== 0) process.exit(result.status ?? 1);
}

function stripPackagedSecrets(dir) {
  if (!fs.existsSync(dir)) return;
  for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
    const full = path.join(dir, entry.name);
    if (entry.isDirectory()) {
      stripPackagedSecrets(full);
      continue;
    }
    const name = entry.name;
    const secret =
      name === ".env" ||
      name.startsWith(".env.") ||
      name.endsWith(".pem") ||
      name.endsWith(".db") ||
      name.includes(".db-");
    if (secret) fs.rmSync(full, { force: true });
  }
}

function assertClean(dir) {
  const bad = [];
  const walk = (current) => {
    for (const entry of fs.readdirSync(current, { withFileTypes: true })) {
      const full = path.join(current, entry.name);
      if (entry.isDirectory()) {
        walk(full);
        continue;
      }
      if (
        entry.name === ".env" ||
        entry.name.startsWith(".env.") ||
        entry.name.endsWith(".db") ||
        entry.name.includes(".db-") ||
        entry.name.endsWith(".pem")
      ) {
        bad.push(full);
      }
    }
  };
  walk(dir);
  if (bad.length > 0) {
    console.error("Refusing to build an installer that contains secrets or a database:");
    for (const file of bad) console.error("  " + file);
    process.exit(1);
  }
}

function stageStandalone() {
  const standaloneSrc = path.join(root, ".next", "standalone");
  const serverJs = path.join(standaloneSrc, "server.js");
  if (!fs.existsSync(serverJs)) {
    console.error("next build did not produce .next/standalone/server.js");
    process.exit(1);
  }

  fs.rmSync(stageRoot, { recursive: true, force: true });
  fs.cpSync(standaloneSrc, stage, { recursive: true, dereference: true });

  // Next's standalone trace omits internal modules the server still requires
  // (for example dist/lib/metadata). Ship the full Next package so the
  // installed app can boot without a dev checkout.
  const nextDest = path.join(stage, "node_modules", "next");
  fs.rmSync(nextDest, { recursive: true, force: true });
  fs.cpSync(path.join(root, "node_modules", "next"), nextDest, {
    recursive: true,
    dereference: true,
  });

  const staticSrc = path.join(root, ".next", "static");
  if (!fs.existsSync(staticSrc)) {
    console.error("next build did not produce .next/static");
    process.exit(1);
  }
  fs.cpSync(staticSrc, path.join(stage, ".next", "static"), { recursive: true, dereference: true });

  const publicSrc = path.join(root, "public");
  if (fs.existsSync(publicSrc)) {
    fs.cpSync(publicSrc, path.join(stage, "public"), { recursive: true, dereference: true });
  }

  const sqlite = path.join(stage, "node_modules", "better-sqlite3");
  if (!fs.existsSync(sqlite)) {
    console.error("Standalone output is missing better-sqlite3. The Windows app cannot open its database.");
    process.exit(1);
  }

  stripPackagedSecrets(stage);
  assertClean(stage);
}

function copyWindowsNode() {
  const source = process.execPath;
  if (path.extname(source).toLowerCase() !== ".exe") {
    console.error("The Windows build must run under node.exe so the installer can ship that same Node.");
    process.exit(1);
  }
  fs.copyFileSync(source, path.join(stage, "node.exe"));
  for (const name of ["LICENSE", "LICENSE.txt"]) {
    const license = path.join(path.dirname(source), name);
    if (fs.existsSync(license)) {
      fs.copyFileSync(license, path.join(stage, "NODE_LICENSE.txt"));
      break;
    }
  }
}

const nextBin = path.join(root, "node_modules", "next", "dist", "bin", "next");
if (!fs.existsSync(nextBin)) {
  console.error("Dependencies are missing. Run npm install first.");
  process.exit(1);
}

run(process.execPath, [nextBin, "build"]);
stageStandalone();

if (process.platform !== "win32") {
  console.error(
    [
      "",
      "Anchored's Windows Setup.exe was not created.",
      `This machine is ${process.platform}. Build the installer on Windows:`,
      "  npm install",
      "  npm run desktop:dist",
      "The Setup file will be dist-desktop\\Anchored-Setup-1.0.0.exe",
      "The standalone server (without the installer) is in desktop-bundle/standalone.",
      "",
    ].join("\n")
  );
  process.exit(1);
}

copyWindowsNode();
assertClean(stage);

const builder = path.join(root, "node_modules", "electron-builder", "cli.js");
if (!fs.existsSync(builder)) {
  console.error("electron-builder is not installed. Run npm install first.");
  process.exit(1);
}

run(process.execPath, [builder, "--win", "nsis", "--x64", "--publish", "never"], {
  CSC_IDENTITY_AUTO_DISCOVERY: "false",
});

console.log("\nWindows installer is in dist-desktop\\ (Anchored-Setup-*.exe). It was not uploaded.");
