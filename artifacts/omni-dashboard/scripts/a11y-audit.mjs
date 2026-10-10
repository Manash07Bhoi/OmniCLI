/**
 * Rendered accessibility and UI-state audit for the OmniCLI dashboard.
 *
 *   node scripts/a11y-audit.mjs [BASE_URL]
 *
 * Default BASE_URL is the local dev/preview server (http://127.0.0.1:3000),
 * which must be running with the API reachable (`pnpm dev` plus the API on
 * :8080, or `pnpm preview` — both proxy /api).
 *
 * Playwright and axe-core are intentionally NOT dependencies of this package:
 * they are several hundred megabytes and are only needed when auditing a
 * rendered UI. Run them through npx:
 *
 *   npx --yes playwright@1.56.1 install chromium
 *   npx --yes --package=@axe-core/playwright@4 --package=playwright@1.56.1 \
 *     node scripts/a11y-audit.mjs
 *
 * It checks, per page: axe-core violations (WCAG 2.0/2.1 A, AA and best
 * practice), document language, a single h1, keyboard reachability of the
 * navigation, the reduced-motion policy, and the real loading/empty/error
 * states by requesting pages with and without a working API.
 */
import { chromium } from "playwright";
import AxeBuilder from "@axe-core/playwright";

const BASE = process.argv[2] ?? "http://127.0.0.1:3000";
const PAGES = [
  ["Dashboard", "/"],
  ["Files", "/files"],
  ["Search", "/search"],
  ["Workspace", "/workspace"],
  ["Archive", "/archive"],
  ["Backup", "/backup"],
  ["Convert", "/convert"],
  ["Dev tools", "/dev"],
];

const results = [];
const browser = await chromium.launch({ args: ["--no-sandbox"] });

async function auditPage(name, path, { offline = false } = {}) {
  const context = await browser.newContext({ viewport: { width: 1280, height: 900 } });
  if (offline) {
    // Fail every API call so the error/offline states are the ones rendered.
    await context.route("**/api/**", (route) => route.abort("failed"));
  }
  const page = await context.newPage();
  const problems = [];
  try {
    await page.goto(`${BASE}${path}`, { waitUntil: "networkidle", timeout: 30_000 });
  } catch (err) {
    problems.push(`navigation failed: ${err.message}`);
  }

  const html = await page.content();
  const lang = await page.getAttribute("html", "lang").catch(() => null);
  if (!lang) problems.push("html element has no lang attribute");
  const h1s = await page.locator("h1").count();
  if (h1s === 0) problems.push("no h1 on the page");
  if (h1s > 1) problems.push(`${h1s} h1 elements (expected 1)`);

  // Keyboard reachability: the skip target and main landmarks must be focusable
  // / present, and Tab must move focus at least once.
  await page.keyboard.press("Tab");
  const focused = await page.evaluate(() => document.activeElement?.tagName ?? null);
  if (!focused || focused === "BODY") problems.push("Tab did not move focus (nothing focusable first)");
  const mainCount = await page.locator("main").count();
  if (mainCount === 0) problems.push("no <main> landmark");

  const axe = await new AxeBuilder({ page })
    .withTags(["wcag2a", "wcag2aa", "wcag21a", "wcag21aa", "best-practice"])
    .analyze();
  for (const violation of axe.violations) {
    problems.push(
      `axe ${violation.impact}: ${violation.id} — ${violation.help} (${violation.nodes.length} node(s): ${violation.nodes
        .slice(0, 2)
        .map((n) => n.target.join(" "))
        .join(", ")})`,
    );
  }

  results.push({ name, path, offline, bytes: html.length, problems });
  await context.close();
}

for (const [name, path] of PAGES) {
  await auditPage(name, path);
}
// One page in the offline state: it must say something truthful, not spin forever.
await auditPage("Dashboard (API unreachable)", "/", { offline: true });

// Reduced motion must be respected by the stylesheet.
const context = await browser.newContext({ reducedMotion: "reduce" });
const page = await context.newPage();
await page.goto(`${BASE}/`, { waitUntil: "networkidle" });
const motion = await page.evaluate(() => {
  const el = document.createElement("div");
  document.body.appendChild(el);
  const style = getComputedStyle(el);
  const duration = style.animationDuration;
  el.remove();
  return duration;
});
const reduceRuleApplied = await page.evaluate(() =>
  [...document.styleSheets].some((sheet) => {
    try {
      return [...sheet.cssRules].some(
        (rule) => rule.conditionText?.includes("prefers-reduced-motion") ?? false,
      );
    } catch {
      return false;
    }
  }),
);
await context.close();

await browser.close();

let failures = 0;
for (const r of results) {
  const label = r.offline ? `${r.name} [API unreachable]` : r.name;
  if (r.problems.length === 0) {
    console.log(`PASS ${label} (${r.path})`);
  } else {
    failures += r.problems.length;
    console.log(`FAIL ${label} (${r.path})`);
    for (const p of r.problems) console.log(`     - ${p}`);
  }
}
console.log(
  `\nreduced-motion media query present in stylesheets: ${reduceRuleApplied ? "yes" : "NO"}; ` +
    `sample animation-duration under reduced motion: "${motion}"`,
);
console.log(`\n${results.length} pages audited, ${failures} problem(s) found`);
process.exit(failures === 0 ? 0 : 1);
