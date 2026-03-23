#!/usr/bin/env node

const { execFileSync } = require("child_process");
const path = require("path");
const os = require("os");
const fs = require("fs");

const platform = os.platform();
const arch = os.arch();

const BINARY_NAME = platform === "win32" ? "b0.exe" : "b0";
const binaryPath = path.join(__dirname, BINARY_NAME);

if (!fs.existsSync(binaryPath)) {
  console.error(
    `Box0 binary not found at ${binaryPath}\n` +
    `Run 'npm install' or 'npx box0' to download it.\n` +
    `Platform: ${platform}-${arch}`
  );
  process.exit(1);
}

try {
  execFileSync(binaryPath, process.argv.slice(2), { stdio: "inherit" });
} catch (e) {
  if (e.status !== undefined) {
    process.exit(e.status);
  }
  throw e;
}
