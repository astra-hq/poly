import { defineConfig, devices } from '@playwright/test';

/**
 * Playwright configuration for Poly frontend e2e tests.
 *
 * The Next.js dev server runs on port 3118. The config automatically
 * starts `pnpm run dev` before tests and tears it down after.
 *
 * Tests live under tests/e2e/*.spec.ts and use mocked Tauri invoke
 * (no real Rust backend needed).
 */
export default defineConfig({
  testDir: './e2e',
  timeout: 30_000,
  expect: {
    timeout: 10_000,
  },
  fullyParallel: true,
  forbidOnly: !!process.env.CI,
  retries: process.env.CI ? 2 : 0,
  workers: process.env.CI ? 1 : undefined,
  reporter: [
    ['html', { outputFolder: '.playwright-report' }],
    ['json', { outputFile: '.playwright-results/results.json' }],
  ],
  use: {
    baseURL: 'http://localhost:3118',
    trace: 'on-first-retry',
    screenshot: 'only-on-failure',
  },

  projects: [
    {
      name: 'chromium',
      use: { ...devices['Desktop Chrome'] },
    },
  ],

  webServer: {
    command: 'pnpm run dev',
    url: 'http://localhost:3118',
    reuseExistingServer: !process.env.CI,
    timeout: 60_000,
    cwd: '.',
  },
});
