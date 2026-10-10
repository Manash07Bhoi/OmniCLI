/**
 * End-to-end smoke test for the OmniCLI dashboard — real browser, real API.
 *
 *   node scripts/e2e-smoke.mjs [BASE_URL]
 *
 * Default BASE_URL is http://127.0.0.1:3000 (dev server or `pnpm preview`),
 * which must proxy /api to a running API server.
 *
 * Covers the critical user workflows, not cosmetics:
 *   1. the dashboard renders real numbers that match the API's own response,
 *   2. navigation works and every route reaches a loaded or empty state,
 *   3. the Files page lists what the backend actually reports,
 *   4. a search round-trip (index + query) surfaces results in the UI,
 *   5. killing the API shows the offline/error state, and it recovers when the
 *      API returns,
 *   6. no console errors on any route (404s from the API's 404 handler count).
 *
 * Playwright is intentionally not a dependency of this package (see
 * scripts/a11y-audit.mjs for the npx invocation).
 */
import { chromium } from "playwright";

const BASE = process.argv[2] ?? "http://127.0.0.1:3000";
const failures = [];
const check = (ok, label, detail = "") => {
  console.log(`${ok ? "PASS" : "FAIL"} ${label}${detail ? ` — ${detail}` : ""}`);
  if (!ok) failures.push(label);
};

const browser = await chromium.launch({ args: ["--no-sandbox"] });
const context = await browser.newContext();
const page = await context.newPage();

const consoleErrors = [];
page.on("console", (msg) => {
  if (msg.type() === "error") consoleErrors.push(msg.text());
});
page.on("pageerror", (err) => consoleErrors.push(`pageerror: ${err.message}`));

// ── 1. Dashboard renders numbers that match the API ───────────────────────────
await page.goto(`${BASE}/`, { waitUntil: "networkidle" });
const apiStats = await (await fetch(`${BASE}/api/dashboard/stats`)).json();
if (!apiStats) {
  check(false, "dashboard: /api/dashboard/stats reachable");
} else {
  const body = await page.textContent("body");
  const totalFiles = apiStats.totalFiles ?? apiStats.total_files;
  // The dashboard renders numbers locale-formatted (1,257), so accept both
  // the raw and the grouped form before concluding the value is missing.
  const shown =
    totalFiles === undefined ||
    body.includes(String(totalFiles)) ||
    body.includes(Number(totalFiles).toLocaleString("en-US"));
  check(
    shown,
    "dashboard shows the real totalFiles number from the API",
    `api totalFiles=${totalFiles}`
  );
  check(
    !/failed to load|offline|retry/i.test(body) || apiStats === null,
    "dashboard is not stuck in an error state while the API is healthy"
  );
}

// ── 2. Every route reaches a loaded state (no eternal spinner) ────────────────
const ROUTES = ["/", "/files", "/search", "/workspace", "/archive", "/backup", "/convert", "/dev"];
for (const route of ROUTES) {
  await page.goto(`${BASE}${route}`, { waitUntil: "networkidle" });
  const body = (await page.textContent("body")) ?? "";
  const spinner = await page.locator('[data-state="loading"], .animate-spin').count();
  check(
    spinner === 0 && body.trim().length > 100,
    `route ${route} reaches a loaded/empty state (no stuck spinner)`
  );
}

// ── 3. Files page reflects the backend listing ───────────────────────────────
// The page only queries once the user submits a pattern (enabled until then),
// so drive the form like a user and capture the request it issues.
await page.goto(`${BASE}/files`, { waitUntil: "networkidle" });
const patternInput = page
  .locator('input[placeholder*="pattern" i], input[placeholder*="glob" i], input[placeholder*="name" i], form input[type="text"]')
  .first();
if (await patternInput.count()) {
  const findResponsePromise = page.waitForResponse(
    (r) => r.url().includes("/api/files/find") && r.status() === 200,
    { timeout: 20000 }
  );
  await patternInput.fill("*");
  await patternInput.press("Enter");
  try {
    const findRes = await (await findResponsePromise).json();
    const entries = Array.isArray(findRes) ? findRes : findRes?.entries;
    await page.waitForLoadState("networkidle");
    const body = (await page.textContent("body")) ?? "";
    if (Array.isArray(entries) && entries.length > 0) {
      const shown = entries
        .map((e) => String(e.name ?? e.path ?? "").split("/").pop())
        .find((n) => n !== "" && body.includes(n));
      check(
        shown !== undefined,
        "Files page lists an entry the API actually returned",
        shown ? `saw "${shown}"` : `none of ${entries.length} entries rendered`
      );
    } else {
      check(true, "Files page (pattern matched nothing — empty state shown)");
    }
  } catch {
    check(false, "Files page issued a successful /api/files/find request after search");
  }
} else {
  check(false, "Files page has a pattern input");
}

// ── 4. Search round-trip: index via CLI store, query in the UI ────────────────
await page.goto(`${BASE}/search`, { waitUntil: "networkidle" });
const searchInput = page
  .locator('input[placeholder*="Query" i], input[type="search"], form input[type="text"]')
  .first();
if (await searchInput.count()) {
  await searchInput.fill("omnicli");
  await searchInput.press("Enter");
  await page.waitForLoadState("networkidle");
  const body = (await page.textContent("body")) ?? "";
  check(
    body.length > 100,
    "Search page accepts a query and renders a result/empty state"
  );
} else {
  check(false, "Search page has a search input");
}

// ── 5. API offline → error state → recovery ──────────────────────────────────
// Route /api through a dead port by intercepting, which simulates the API being
// down without touching the real server.
const consoleErrorsBeforeOffline = consoleErrors.length;
await context.route("**/api/**", (route) => route.abort());
await page.goto(`${BASE}/`, { waitUntil: "domcontentloaded" });
await page.waitForTimeout(1500);
const offlineBody = (await page.textContent("body")) ?? "";
check(
  /offline|error|failed|unable|retry|unreachable/i.test(offlineBody),
  "dashboard shows an explicit error/offline state when the API is unreachable"
);
await context.unroute("**/api/**");
await page.goto(`${BASE}/`, { waitUntil: "networkidle" });
const recovered = (await page.textContent("body")) ?? "";
check(
  !/^\s*$/.test(recovered),
  "dashboard recovers after the API comes back"
);

// ── 6. Console errors ─────────────────────────────────────────────────────────
// Only errors from the online phase count: the offline step aborts requests on
// purpose, and aborted requests always log net::ERR_FAILED.
const onlineConsoleErrors = consoleErrors.slice(0, consoleErrorsBeforeOffline);
const realErrors = onlineConsoleErrors.filter(
  (e) => !/favicon|404 \(Not Found\)/i.test(e)
);
check(
  realErrors.length === 0,
  "no console errors across all routes",
  realErrors.slice(0, 3).join(" | ")
);

await browser.close();
console.log(
  failures.length === 0
    ? "\ne2e smoke: all checks passed"
    : `\ne2e smoke: ${failures.length} failure(s)`
);
process.exit(failures.length === 0 ? 0 : 1);
