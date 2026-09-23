import { cp, mkdir, readdir, rm, writeFile } from "node:fs/promises";
import { join } from "node:path";

const target = "target/dx/az-memory-frontend/release/web/public";
const output = "dist/frontend";
await rm(output, { recursive: true, force: true });
await mkdir(output, { recursive: true });
await cp(target, output, { recursive: true });
const assets = await readdir(join(output, "assets")).catch(() => []);
const entry = assets.find((name) => name.endsWith(".js"));
if (!entry) throw new Error("Dioxus 前端资源未生成");
const html = `<!doctype html><html lang="zh-CN"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>智能体记忆</title></head><body><div id="main"></div><script type="module" src="/assets/${entry}"></script></body></html>`;
await writeFile(join(output, "index.html"), html);
