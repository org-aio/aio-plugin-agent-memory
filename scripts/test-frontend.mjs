import { chromium } from "playwright";
import http from "node:http";
import { readFile } from "node:fs/promises";
import { extname, join } from "node:path";

const root = "dist/frontend";
const base_path = "/api/runtime/components/assets/TESTDIGEST/";
const mime = {
  ".html": "text/html",
  ".js": "text/javascript",
  ".wasm": "application/wasm",
  ".css": "text/css",
  ".ttf": "font/ttf",
  ".woff": "font/woff",
  ".woff2": "font/woff2",
  ".svg": "image/svg+xml",
  ".png": "image/png",
};

const server = http.createServer(async (request, response) => {
  const url = request.url.split("?")[0];
  if (url === "/" || url === base_path || url === `${base_path}index.html`) {
    const html = (await readFile(join(root, "index.html"), "utf8")).replace(
      "<head>",
      `<head><base href="${base_path}">`,
    );
    response.setHeader("content-type", "text/html");
    response.end(html);
    return;
  }
  if (url.startsWith(base_path)) {
    const relative = url.slice(base_path.length) || "index.html";
    try {
      response.setHeader(
        "content-type",
        mime[extname(relative)] || "application/octet-stream",
      );
      response.end(await readFile(join(root, relative)));
    } catch {
      response.statusCode = 404;
      response.end("not found");
    }
    return;
  }
  response.statusCode = 404;
  response.end("not found");
});

await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
const port = server.address().port;
const executablePath =
  process.env.PLAYWRIGHT_CHROMIUM_EXECUTABLE_PATH ||
  "/Users/zjarlin/Library/Caches/ms-playwright/chromium-1243/chrome-mac-arm64/Google Chrome for Testing.app/Contents/MacOS/Google Chrome for Testing";
const browser = await chromium.launch({ headless: true, executablePath });

try {
  const page = await browser.newPage({ viewport: { width: 1440, height: 900 } });
  const calls = [];
  await page.addInitScript(() => {
    window.__aioMemoryCalls = [];
    window.__aioMemorySources = [];
    window.aioPlugin = {
      async json(method, path, body) {
        window.__aioMemoryCalls.push({ method, path, body });
        if (path === "/spaces") {
          return [
            {
              id: "space-1",
              title: "个人空间",
              personal: true,
              role: "OWNER",
              modelBinding: null,
            },
          ];
        }
        if (path.startsWith("/graph")) {
          return { nodes: [], edges: [], total: 0, truncated: false };
        }
        if (path.startsWith("/sources")) {
          return { sources: window.__aioMemorySources, truncated: false };
        }
        if (path.startsWith("/secrets")) return [];
        if (path.startsWith("/import")) {
          window.__aioMemorySources = [
            {
              id: "source-1",
              spaceId: "space-1",
              text: body.text,
              status: "pending",
              createdBy: "user-1",
              updatedAt: 1,
              secrets: [],
              error: null,
            },
          ];
          return {
            source: {
              id: "source-1",
              title: body.text,
              kind: "SOURCE",
              content: body.text,
              url: "",
              tags: [],
              version: 1,
              updatedAt: 1,
              aliases: [],
            },
            linkedNodes: 0,
          };
        }
        throw new Error(`unexpected memory request: ${path}`);
      },
      async copy() {},
    };
  });

  await page.goto(`http://127.0.0.1:${port}${base_path}index.html`, {
    waitUntil: "networkidle",
    timeout: 30_000,
  });
  await page.getByRole("heading", { name: "随心记", exact: true }).waitFor();
  const input = page.getByRole("textbox", { name: "随心记内容" });
  await input.fill("记一下：明天检查记忆图谱");
  await page.getByRole("button", { name: "保存随心记" }).click();
  await page
    .locator("article strong")
    .filter({ hasText: "记一下：明天检查记忆图谱" })
    .waitFor();

  calls.push(...(await page.evaluate(() => window.__aioMemoryCalls)));
  const imported = calls.find((call) => call.path.startsWith("/import"));
  if (!imported) throw new Error("未发出 import 请求");
  if (imported.body.text !== "记一下：明天检查记忆图谱") {
    throw new Error("import 请求正文不匹配");
  }
  const refreshes = calls.filter((call) => call.path.startsWith("/sources"));
  if (refreshes.length < 2) throw new Error("保存后未刷新来源列表");
  console.log("随心记 frontend passed");
} finally {
  await browser.close();
  server.close();
}
