#!/usr/bin/env node
import { execSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const triple =
  process.env.TAURI_ENV_TARGET_TRIPLE?.trim() ||
  execSync('rustc --print host-tuple', { cwd: root }).toString().trim();
const ext = process.platform === 'win32' ? '.exe' : '';

const targetFlag = triple ? ` --target ${triple}` : '';
execSync(`cargo build --release -p chronicle-daemon${targetFlag}`, {
  cwd: root,
  stdio: 'inherit',
});

const src = path.join(root, 'target', triple, 'release/chronicle-daemon') + ext;
const srcFallback = path.join(root, 'target/release/chronicle-daemon') + ext;
const from = fs.existsSync(src) ? src : srcFallback;
const focusSrc =
  (fs.existsSync(path.join(root, 'target', triple, 'release/chronicle-focus-monitor'))
    ? path.join(root, 'target', triple, 'release/chronicle-focus-monitor')
    : path.join(root, 'target/release/chronicle-focus-monitor')) + ext;
const destDir = path.join(root, 'src-tauri/binaries');
const dest = path.join(destDir, `chronicle-daemon-${triple}${ext}`);
const focusDest = path.join(destDir, `chronicle-focus-monitor-${triple}${ext}`);

fs.mkdirSync(destDir, { recursive: true });
fs.copyFileSync(from, dest);
fs.chmodSync(dest, 0o755);
console.log(`Bundled daemon (${triple}) → ${dest}`);
if (fs.existsSync(focusSrc)) {
  fs.copyFileSync(focusSrc, focusDest);
  fs.chmodSync(focusDest, 0o755);
  console.log(`Bundled focus monitor (${triple}) → ${focusDest}`);
} else {
  console.warn(`Focus monitor binary missing at ${focusSrc} — run cargo build -p chronicle-daemon`);
}
