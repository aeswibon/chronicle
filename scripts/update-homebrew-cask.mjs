#!/usr/bin/env node
/**
 * Update Casks/chronicle.rb version and sha256 from release checksum files.
 * Invoked by .github/workflows/release.yml after DMG artifacts are built.
 *
 * Usage: node scripts/update-homebrew-cask.mjs <version> [checksums-arm64.txt] [checksums-x64.txt]
 */
import fs from 'node:fs';
import path from 'node:path';

const version = process.argv[2];
const armFile = process.argv[3] ?? 'checksums-arm64.txt';
const x64File = process.argv[4] ?? 'checksums-x64.txt';
const caskPath = path.join('Casks', 'chronicle.rb');

if (!version || !/^\d+\.\d+\.\d+/.test(version)) {
  console.error('Usage: node scripts/update-homebrew-cask.mjs <semver> [arm-checksums] [x64-checksums]');
  process.exit(1);
}

function readSha256(file) {
  if (!fs.existsSync(file)) {
    console.error(`Missing checksum file: ${file}`);
    process.exit(1);
  }
  const hash = fs.readFileSync(file, 'utf8').trim().split(/\s+/)[0];
  if (!/^[a-f0-9]{64}$/.test(hash)) {
    console.error(`Invalid sha256 in ${file}: ${hash}`);
    process.exit(1);
  }
  return hash;
}

const arm = readSha256(armFile);
const intel = readSha256(x64File);

if (!fs.existsSync(caskPath)) {
  console.error(`Missing cask: ${caskPath}`);
  process.exit(1);
}

let cask = fs.readFileSync(caskPath, 'utf8');
cask = cask.replace(/^(\s*version\s+)"[^"]+"/m, `$1"${version}"`);
cask = cask.replace(
  /^(\s*sha256\s+arm:\s+)"[a-f0-9]{64}",\s*\n\s*intel:\s+"[a-f0-9]{64}"/m,
  `$1"${arm}",\n         intel: "${intel}"`,
);

fs.writeFileSync(caskPath, cask);
console.log(`Updated ${caskPath} for v${version}`);
console.log(`  arm:   ${arm}`);
console.log(`  intel: ${intel}`);
