#!/usr/bin/env node
/**
 * Playwright E2E wrapper script.
 *
 * pnpm run test:e2e -- <args> forwards a literal '--' as the first argument.
 * Playwright interprets that '--' as "end of options", which causes it to
 * treat subsequent file paths as filter patterns rather than test files.
 * When the filter doesn't match anything, Playwright falls back to running
 * every test in the testDir.
 *
 * This wrapper strips a leading '--' before invoking Playwright so that
 * file paths are resolved correctly.
 *
 * Usage:
 *   pnpm run test:e2e                    # runs all e2e tests
 *   pnpm run test:e2e -- e2e/glossary-settings.spec.ts   # runs only that file
 */

const { execSync } = require('child_process');

let args = process.argv.slice(2);

// Strip a leading literal '--' that pnpm forwards when using
//   pnpm run <script> -- <args>
if (args[0] === '--') {
  args = args.slice(1);
}

const cmd = ['npx', 'playwright', 'test', ...args].join(' ');

console.log(`Running: ${cmd}`);

try {
  execSync(cmd, { stdio: 'inherit' });
} catch (err) {
  process.exit(err.status || 1);
}
