import { describe, it, expect } from "vitest";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const WEB_ROOT = path.resolve(__dirname, "..");
const INDEX_HTML_PATH = path.join(WEB_ROOT, "index.html");
const MAIN_CSS_PATH = path.join(WEB_ROOT, "src", "styles", "main.css");
const SRC_DIR = path.join(WEB_ROOT, "src");

function getAllTsFiles(dir: string): string[] {
  const files: string[] = [];
  for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
    const fullPath = path.join(dir, entry.name);
    if (entry.isDirectory()) {
      if (entry.name !== "wasm") {
        files.push(...getAllTsFiles(fullPath));
      }
    } else if (entry.name.endsWith(".ts") && !entry.name.endsWith(".test.ts") && !entry.name.endsWith(".d.ts")) {
      files.push(fullPath);
    }
  }
  return files;
}

describe("Web Boot & Markup Smoke Guard", () => {
  it("ensures index.html contains all critical layout and HUD elements", () => {
    const html = fs.readFileSync(INDEX_HTML_PATH, "utf-8");

    const criticalElements = [
      "loading",
      "loading-text",
      "viewport-wrap",
      "world-canvas",
      "world-gpu-canvas",
      "world-input-layer",
      "minimap-wrap",
      "minimap-canvas",
      "status-bar",
      "st-x",
      "st-y",
      "st-terrain",
      "st-alt",
      "st-moist",
      "st-temp",
      "st-renderer",
      "st-route",
      "st-zoom",
      "sidebar",
      "simulation-hud",
      "hud-btn-pause",
      "hud-btn-play",
      "hud-btn-step",
      "hud-tick",
      "hud-pop",
      "hud-hash",
      "hud-hash-val",
      "entity-inspector",
      "tile-inspector",
      "saved-worlds-list",
      "btn-save-world",
      "btn-export-world",
      "world-name-input",
      "import-file-input",
      "camera-controls",
      "btn-zoom-in",
      "btn-zoom-out",
      "btn-zoom-fit",
      "btn-toggle-grid",
    ];

    const missing = criticalElements.filter((id) => !html.includes(`id="${id}"`));
    expect(
      missing,
      `Critical elements missing from index.html: ${missing.join(", ")}`,
    ).toEqual([]);
  });

  it("ensures all document.getElementById queries in source code exist in index.html", () => {
    const html = fs.readFileSync(INDEX_HTML_PATH, "utf-8");
    const tsFiles = getAllTsFiles(SRC_DIR);

    // Dynamically created elements that do not originate in index.html
    const dynamicallyCreatedIds = new Set(["regen-toast"]);

    const idRegex = /document\.getElementById\(\s*["']([^"']+)["']\s*\)/g;
    const queryIdRegex = /document\.querySelector(?:<[^>]+>)?\(\s*["']#([a-zA-Z0-9_-]+)["']\s*\)/g;

    const referencedIds = new Map<string, string[]>();

    for (const file of tsFiles) {
      const content = fs.readFileSync(file, "utf-8");
      const relPath = path.relative(WEB_ROOT, file);

      let match: RegExpExecArray | null;
      while ((match = idRegex.exec(content)) !== null) {
        const id = match[1];
        if (!dynamicallyCreatedIds.has(id)) {
          const callers = referencedIds.get(id) || [];
          callers.push(relPath);
          referencedIds.set(id, callers);
        }
      }
      while ((match = queryIdRegex.exec(content)) !== null) {
        const id = match[1];
        if (!dynamicallyCreatedIds.has(id)) {
          const callers = referencedIds.get(id) || [];
          callers.push(relPath);
          referencedIds.set(id, callers);
        }
      }
    }

    const missing: { id: string; callers: string[] }[] = [];
    for (const [id, callers] of referencedIds) {
      if (!html.includes(`id="${id}"`)) {
        missing.push({ id, callers });
      }
    }

    if (missing.length > 0) {
      const details = missing
        .map((m) => `  - #${m.id} referenced in: ${m.callers.join(", ")}`)
        .join("\n");
      expect.fail(
        `TypeScript code references DOM element IDs not present in index.html:\n${details}`,
      );
    }
  });

  it("ensures every tab button has a corresponding tab panel in index.html", () => {
    const html = fs.readFileSync(INDEX_HTML_PATH, "utf-8");
    const tabRegex = /data-tab="([a-zA-Z0-9_-]+)"/g;

    const tabNames: string[] = [];
    let match: RegExpExecArray | null;
    while ((match = tabRegex.exec(html)) !== null) {
      tabNames.push(match[1]);
    }

    expect(tabNames.length).toBeGreaterThan(0);

    for (const tab of tabNames) {
      const hasPanel =
        html.includes(`id="tab-${tab}"`) ||
        html.includes(`data-tab-content="${tab}"`);
      expect(
        hasPanel,
        `Tab button [data-tab="${tab}"] has no corresponding #tab-${tab} panel in index.html`,
      ).toBe(true);
    }
  });

  it("verifies main.css brace balance and guarantees critical layout rules are at top-level depth 0", () => {
    const css = fs.readFileSync(MAIN_CSS_PATH, "utf-8");

    let depth = 0;
    let maxDepth = 0;
    let inComment = false;
    let inString: string | null = null;

    const criticalTopLevelRules = [
      "#viewport-wrap",
      "#status-bar",
      "#minimap-wrap",
      "#tooltip",
    ];
    const ruleDepths = new Map<string, number>();

    let currentSelector = "";

    for (let i = 0; i < css.length; i++) {
      const char = css[i];
      const nextChar = css[i + 1];

      // Comment handling
      if (inComment) {
        if (char === "*" && nextChar === "/") {
          inComment = false;
          i++;
        }
        continue;
      }
      if (char === "/" && nextChar === "*") {
        inComment = true;
        i++;
        continue;
      }

      // String handling
      if (inString) {
        if (char === inString && css[i - 1] !== "\\") {
          inString = null;
        }
        continue;
      }
      if (char === '"' || char === "'") {
        inString = char;
        continue;
      }

      if (char === "{") {
        const sel = currentSelector.trim();
        for (const rule of criticalTopLevelRules) {
          if (sel.includes(rule) && !ruleDepths.has(rule)) {
            ruleDepths.set(rule, depth);
          }
        }
        currentSelector = "";
        depth++;
        if (depth > maxDepth) maxDepth = depth;
      } else if (char === "}") {
        currentSelector = "";
        depth--;
        expect(
          depth >= 0,
          `CSS syntax error: Unexpected closing brace '}' at character ${i}`,
        ).toBe(true);
      } else if (depth === 0) {
        currentSelector += char;
      }
    }

    expect(depth, `CSS syntax error: Unclosed braces. Final depth is ${depth}`).toBe(0);
    expect(maxDepth).toBeGreaterThan(0);

    // Verify all critical layout selectors are defined at depth 0
    for (const rule of criticalTopLevelRules) {
      const foundDepth = ruleDepths.get(rule);
      expect(
        foundDepth,
        `Critical layout rule "${rule}" was not found or was nested inside another rule! (Depth: ${foundDepth})`,
      ).toBe(0);
    }
  });
});
