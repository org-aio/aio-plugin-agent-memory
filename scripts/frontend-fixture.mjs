import http from "node:http";
import { readFile } from "node:fs/promises";
import { extname, resolve, sep } from "node:path";
import { pathToFileURL } from "node:url";

// 仅本机验收使用：合成资料和 SDK 替身不接触真实账号或数据库。
function installFixture() {
  const readonly = new URLSearchParams(location.search).has("readonly");
  const samples = [
    "# 周末阅读\n\n把零散想法留下来，再慢慢找到它们之间的联系。\n\n- 读完第三章\n- 整理两个有意思的问题\n\n> 一条记录，只记一件事。",
    "## 小项目清单\n\n| 事项 | 进度 |\n| --- | --- |\n| 整理书架 | 已完成 |\n| 做一本阅读手册 | 进行中 |\n\n下周继续补充。",
    "## 一段代码\n\n```rust\nlet notes = vec![\"想法\", \"书摘\"];\nfor note in notes {\n    println!(\"{note}\");\n}\n```\n\n保留代码格式，回头还看得懂。",
    "午后散步时想到：给每个项目留一页日志，记录今天做了什么、下次从哪里继续。",
  ];
  window.__memoryCalls = [];
  window.__memoryFail = null;
  window.__memorySources = Array.from({ length: 28 }, (_, index) => ({
    id: `note-${index + 1}`, spaceId: "space-1", title: `记录 ${index + 1}`,
    text: samples[index % samples.length] + (index >= 4 ? `\n\n归档 ${index + 1}` : ""),
    origin: index % 3 === 0 ? "note" : "import", status: index % 4 === 0 ? "pending" : "complete",
    createdBy: "fixture-user", updatedAt: Date.now() - index * 3600_000,
    secrets: [], error: null, version: 1, canEdit: !readonly, canDelete: !readonly,
  }));
  let serial = 100;
  const requests = new Map();
  window.aioPlugin = {
    async json(method, path, body) {
      window.__memoryCalls.push({ method, path, body });
      await new Promise((resolve) => setTimeout(resolve, 30));
      if (window.__memoryFail === `${method} ${path.split("?")[0]}`) {
        window.__memoryFail = null;
        throw new Error(method === "PUT" ? "版本已变化，请重新打开记录后合并修改" : "请求失败，请重试");
      }
      const url = new URL(path, "https://fixture.invalid");
      const route = url.pathname;
      if (route === "/spaces") return [
        { id: "space-1", title: "个人记忆", personal: true, role: readonly ? "READER" : "OWNER", modelBinding: null },
        { id: "space-2", title: "项目笔记", personal: false, role: "OWNER", modelBinding: null },
      ];
      if (route === "/graph") return { nodes: [], edges: [], total: 0, truncated: false };
      if (route === "/secrets") return [];
      if (route === "/sources") {
        const q = (url.searchParams.get("query") || "").toLowerCase();
        const status = url.searchParams.get("status");
        const offset = Number(url.searchParams.get("offset") || 0);
        const limit = Number(url.searchParams.get("limit") || 200);
        const sources = window.__memorySources.filter((source) =>
          source.spaceId === (url.searchParams.get("spaceId") || "space-1") &&
          source.text.toLowerCase().includes(q) && (!status || source.status === status));
        return { sources: sources.slice(offset, offset + limit), total: sources.length, truncated: offset + limit < sources.length };
      }
      if ((route === "/capture" || route === "/import") && method === "POST") {
        if (readonly) throw new Error("当前空间只读");
        if (requests.has(body.requestId)) return requests.get(body.requestId);
        const source = {
          id: `note-${serial++}`, spaceId: body.spaceId || url.searchParams.get("spaceId") || "space-1",
          text: body.text, title: body.title || body.text.split("\n")[0], status: "pending", origin: "note",
          createdBy: "fixture-user", updatedAt: Date.now(), version: 1, secrets: [], error: null,
          canEdit: true, canDelete: true,
        };
        window.__memorySources.unshift(source);
        requests.set(body.requestId, source);
        return route === "/import" ? { source: { ...source, kind: "SOURCE", content: source.text, url: "", tags: [], aliases: [] }, linkedNodes: 0 } : source;
      }
      const match = route.match(/^\/sources\/([^/]+)(\/original)?$/);
      if (match) {
        const source = window.__memorySources.find((item) => item.id === match[1]);
        if (!source) throw new Error("记录不存在");
        if (match[2] && method === "POST") return { value: source.text };
        if (method === "GET") return source;
        if (readonly) throw new Error("当前空间只读");
        if (method === "PUT") {
          if (body.version !== source.version) throw new Error("版本已变化");
          Object.assign(source, { text: body.text, version: source.version + 1, updatedAt: Date.now(), status: "pending" });
          return source;
        }
        if (method === "DELETE") {
          window.__memorySources = window.__memorySources.filter((item) => item.id !== source.id);
          return null;
        }
      }
      throw new Error(`unexpected fixture request: ${method} ${path}`);
    },
    async copy() {},
  };
}

const assetRoot = resolve("dist/frontend");
const basePath = "/api/runtime/components/assets/TESTDIGEST/";
const mime = { ".html": "text/html", ".js": "text/javascript", ".wasm": "application/wasm", ".css": "text/css", ".ttf": "font/ttf", ".woff": "font/woff", ".woff2": "font/woff2", ".svg": "image/svg+xml", ".png": "image/png" };

export async function startFixture(port = 0) {
  const server = http.createServer(async (request, response) => {
    try {
      const origin = `http://127.0.0.1:${server.address().port}`;
      const url = new URL(request.url, origin);
      response.setHeader("access-control-allow-origin", "*");
      if (url.pathname === "/favicon.ico") { response.writeHead(204).end(); return; }
      if (url.pathname === "/") {
        response.setHeader("content-type", "text/html; charset=utf-8");
        response.end(`<!doctype html><html lang="zh-CN"><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>记忆 - 本地预览</title><style>*{box-sizing:border-box}body{margin:0;font:14px system-ui;color:#202326;background:#fff}header{height:56px;border-bottom:1px solid #e6e8eb;display:flex;align-items:center;gap:32px;padding:0 24px}header b{font-size:18px}header span{color:#60656d}.shell{display:flex;height:calc(100dvh - 56px)}aside{width:196px;border-right:1px solid #e6e8eb;padding:24px 12px;flex-shrink:0}aside p{padding:0 12px;margin:0 0 20px;color:#60656d}aside div{padding:10px 12px;background:#eef2f3;border-radius:6px;font-weight:600}iframe{width:100%;height:100%;border:0;min-width:0}@media(max-width:700px){aside{display:none}header{padding:0 16px;height:48px}.shell{height:calc(100dvh - 48px)}}</style><header><b>AIO</b><span>工作空间 / 记忆</span></header><div class="shell"><aside><p>智能体</p><div>记忆</div></aside><iframe title="记忆" sandbox="allow-scripts allow-forms" src="${basePath}index.html${url.search}"></iframe></div></html>`);
        return;
      }
      if (!url.pathname.startsWith(basePath)) { response.writeHead(404).end(); return; }
      const relative = url.pathname.slice(basePath.length) || "index.html";
      if (relative === "index.html") {
        const prefix = `${origin}${basePath}`;
        response.setHeader("content-type", "text/html; charset=utf-8");
        response.setHeader("content-security-policy", `sandbox allow-scripts allow-forms; default-src 'none'; script-src 'unsafe-inline' 'unsafe-eval' 'wasm-unsafe-eval' blob: ${prefix}; connect-src ${prefix} blob:; style-src 'unsafe-inline' blob: ${prefix}; img-src data: blob: ${prefix}; font-src data: ${prefix}; object-src 'none'; frame-src 'none'; worker-src blob:; base-uri ${prefix}; form-action 'none'; frame-ancestors 'self'`);
        const html = (await readFile(resolve(assetRoot, relative), "utf8"))
          .replace("<head>", `<head><base href="${prefix}"><script>(${installFixture.toString()})()</script>`);
        response.end(html);
        return;
      }
      const file = resolve(assetRoot, relative);
      if (!file.startsWith(assetRoot + sep)) { response.writeHead(403).end(); return; }
      response.setHeader("content-type", mime[extname(relative)] || "application/octet-stream");
      response.end(await readFile(file));
    } catch { response.writeHead(404).end("not found"); }
  });
  await new Promise((resolve, reject) => { server.once("error", reject); server.listen(port, "127.0.0.1", resolve); });
  return { server, url: `http://127.0.0.1:${server.address().port}` };
}

if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) {
  const { url } = await startFixture(Number(process.env.PORT || 0));
  console.log(`Memory preview (synthetic data): ${url}`);
}
