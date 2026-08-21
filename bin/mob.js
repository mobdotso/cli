#!/usr/bin/env node
import { execFileSync } from "child_process";
import { constants } from "os";
import path from "path";
import { exit } from "process";
import { fileURLToPath } from "url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const binName = process.platform === "win32" ? "mob.exe" : "mob";
const binPath = path.resolve(__dirname, binName);

try {
  execFileSync(binPath, process.argv.slice(2), { stdio: "inherit" });
} catch (e) {
  // The binary is downloaded by npm-install/postinstall.js. If that step was
  // skipped (--ignore-scripts) or failed (unsupported platform, network), the
  // spawn fails with ENOENT; say so instead of exiting 1 with no output.
  if (e.code === "ENOENT") {
    console.error(
      `mob: could not find the CLI binary at ${binPath}\n` +
        `The @mobdotso/cli install step did not complete. Try:\n` +
        `  npm install -g @mobdotso/cli --foreground-scripts\n` +
        `or install the CLI directly:\n` +
        `  curl -fsSL https://mob.so/install.sh | sh`,
    );
    exit(127);
  }

  // Propagate the real exit status so usage errors (clap exits 2) and
  // crashes keep their codes for `set -e` callers.
  if (typeof e.status === "number") {
    exit(e.status);
  }

  // Killed by a signal: report it the way a shell does (128 + signal number).
  if (e.signal) {
    const signum = constants.signals[e.signal];
    exit(signum ? 128 + signum : 1);
  }

  exit(1);
}
