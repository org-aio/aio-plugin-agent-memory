import { createServer } from 'node:http';
import { readFile, realpath } from 'node:fs/promises';
import { spawn } from 'node:child_process';
import { createInterface } from 'node:readline';
import { randomBytes } from 'node:crypto';
import { dirname, resolve, extname, sep } from 'node:path';
import { fileURLToPath } from 'node:url';
import { parse, serialize } from 'parse5';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const platform = resolve(root, process.env.AIO_PLATFORM || '../aio-platform');
const assets = await realpath(resolve(root, 'dist/frontend'));
const port = Number(process.env.PORT || 4191);
const origin = `http://127.0.0.1:${port}`;
const ticket = randomBytes(32).toString('hex');
const child = spawn(resolve(root, 'dev/target/debug/aio-memory-dev'), process.argv.includes('--demo') ? ['--demo'] : [], { cwd: root, env: process.env, stdio: ['pipe', 'pipe', 'inherit'] });
const lines = createInterface({ input: child.stdout });
const waiting = [];
let readyResolve, readyReject;
const ready = new Promise((resolve, reject) => { readyResolve = resolve; readyReject = reject; });
lines.on('line', line => {
  let message; try { message = JSON.parse(line); } catch { console.error(line); return; }
  if (message.ready) { console.log(`Workspace: ${message.workspace}`); readyResolve(); return; }
  const next = waiting.shift();
  if (next) message.error ? next.reject(new Error(message.error)) : next.resolve(message);
});
child.on('error', readyReject);
child.on('exit', code => {
  readyReject(new Error(`Runtime exited: ${code}`));
  waiting.splice(0).forEach(item => item.reject(new Error('Runtime unavailable')));
  process.exitCode = code || 0;
});
const invoke = request => new Promise((resolve, reject) => {
  if (waiting.length >= 8) return reject(new Error('Request queue full'));
  waiting.push({ resolve, reject }); child.stdin.write(JSON.stringify(request) + '\n');
});
await ready;
const types = { '.html': 'text/html; charset=utf-8', '.js': 'text/javascript', '.mjs': 'text/javascript', '.wasm': 'application/wasm', '.otf': 'font/otf', '.json': 'application/json', '.png': 'image/png' };
const shell = `<!doctype html><html lang="zh-CN"><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>Memory · AIO</title><body style="margin:0;overflow:hidden"><iframe title="记忆图谱" id="plugin" src="/assets/index.html" sandbox="allow-scripts" style="display:block;width:100vw;height:100vh;border:0"></iframe><script type="module">
import {mountBridge} from '/bridge/host.mjs';
const dispose = mountBridge(document.getElementById('plugin'), async request => {
 const response = await fetch('/invoke', {method:'POST', headers:{'content-type':'application/json','x-aio-ticket':'${ticket}'}, body:JSON.stringify({...request, body:Array.from(request.body)})});
 if (!response.ok) throw new Error(await response.text()); return response.json();
}); window.addEventListener('pagehide', dispose, {once:true});</script></body></html>`;
const server = createServer(async (request, response) => {
  try {
    if (request.headers.host !== `127.0.0.1:${port}`) { response.writeHead(403).end(); return; }
    const url = new URL(request.url, origin);
    if (url.pathname === '/invoke' && request.method === 'POST') {
      if (request.headers.origin !== origin || request.headers['x-aio-ticket'] !== ticket) { response.writeHead(403).end(); return; }
      let size = 0; const chunks = [];
      for await (const chunk of request) { size += chunk.length; if (size > 3 * 1024 * 1024) { response.writeHead(413).end(); return; } chunks.push(chunk); }
      const data = JSON.parse(Buffer.concat(chunks).toString());
      if (!['GET','POST','PUT','DELETE'].includes(data.method) || typeof data.path !== 'string' || !data.path.startsWith('/') || data.path.startsWith('//') || data.path.length > 2048 || !Array.isArray(data.body) || data.body.length > 512000 || data.body.some(b => !Number.isInteger(b) || b < 0 || b > 255)) { response.writeHead(400).end(); return; }
      const result = await invoke({ method: data.method, path: data.path, body: data.body });
      response.writeHead(200, { 'content-type': 'application/json', 'cache-control': 'no-store' }).end(JSON.stringify(result)); return;
    }
    if (request.method !== 'GET') { response.writeHead(405).end(); return; }
    if (url.pathname === '/') { response.writeHead(200, { 'content-type': 'text/html; charset=utf-8', 'cache-control': 'no-store' }).end(shell); return; }
    if (url.pathname === '/favicon.ico') { response.writeHead(204).end(); return; }
    if (['/bridge/guest.js', '/bridge/host.mjs'].includes(url.pathname)) {
      response.writeHead(200, { 'content-type': 'text/javascript', 'access-control-allow-origin': '*' }).end(await readFile(resolve(platform, 'sdk/web', url.pathname.split('/').at(-1)))); return;
    }
    if (!url.pathname.startsWith('/assets/')) { response.writeHead(404).end(); return; }
    const file = await realpath(resolve(assets, decodeURIComponent(url.pathname.slice(8))));
    if (!file.startsWith(assets + sep)) { response.writeHead(403).end(); return; }
    let bytes = await readFile(file);
    const headers = { 'content-type': types[extname(file)] || 'application/octet-stream', 'access-control-allow-origin': '*', 'cache-control': 'no-cache', 'x-content-type-options': 'nosniff' };
    if (extname(file) === '.html') {
      const doc = parse(bytes.toString());
      const html = doc.childNodes.find(n => n.tagName === 'html');
      const head = html.childNodes.find(n => n.tagName === 'head');
      const script = {nodeName:'script',tagName:'script',namespaceURI:'http://www.w3.org/1999/xhtml',attrs:[{name:'src',value:'/bridge/guest.js'}],childNodes:[],parentNode:head};
      head.childNodes.unshift(script); bytes = Buffer.from(serialize(doc));
      headers['content-security-policy'] = `sandbox allow-scripts; default-src 'none'; script-src ${origin} 'unsafe-inline' 'wasm-unsafe-eval'; connect-src ${origin}/assets/; img-src ${origin}/assets/ data: blob:; font-src ${origin}/assets/; style-src 'unsafe-inline'; worker-src blob:; base-uri 'none'; form-action 'none'`;
    }
    response.writeHead(200, headers).end(bytes);
  } catch (error) { console.error(error.message); if (!response.headersSent) response.writeHead(error.code === 'ENOENT' ? 404 : 500); response.end('Preview request failed'); }
});
server.listen(port, '127.0.0.1', () => console.log(origin));
server.on('error', error => { console.error(error); child.kill(); process.exitCode = 1; });
for (const signal of ['SIGINT', 'SIGTERM']) process.on(signal, () => { child.stdin.end(); server.close(() => process.exit(0)); });
