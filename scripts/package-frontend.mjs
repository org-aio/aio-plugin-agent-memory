import { cp, mkdir, readFile, readdir, rm, writeFile } from "node:fs/promises";
import { join } from "node:path";

const target = "target/dx/az-memory-frontend/release/web/public";
const output = "dist/frontend";
await rm(output, { recursive: true, force: true });
await mkdir(output, { recursive: true });
await cp(target, output, { recursive: true });
const sourceHtml = await readFile(join(target, "index.html"), "utf8");
const entry = sourceHtml.match(/\/\.?\/(assets\/[^"']+\.js)/)?.[1]?.split("/").pop();
if (!entry) throw new Error("Dioxus 前端资源未生成");
// 宿主按 <base href=".../assets/<digest>/"> + CSP(connect-src 限定在 asset 根) 注入组件文档。
// Dioxus 胶水 JS 里写死的 "/./assets/xxx.wasm" 会解析到站点根，被 CSP 拦截导致 wasm 404、
// 页面停在 prepare 空白。改成相对 <base>(即组件 asset 根) 的 "./assets/xxx.wasm"，
// 使 wasm fetch 落在 asset 根内且通过 CSP。
const entryPath = join(output, "assets", entry);
const glue = await readFile(entryPath, "utf8");
const fixedGlue = glue.replace(
  /module_or_path:"\.?\/\.?\/?assets\//g,
  'module_or_path:"./assets/',
).replace(
  /module_or_path:"assets\//g,
  'module_or_path:"./assets/',
);
if (fixedGlue !== glue) await writeFile(entryPath, fixedGlue);
const html = `<!doctype html><html lang="zh-CN"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>智能体记忆</title></head><body><div id="main"></div><script type="module" src="./assets/${entry}"></script></body></html>`;
await writeFile(join(output, "index.html"), html);
