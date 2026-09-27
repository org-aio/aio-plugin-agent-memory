import { chromium } from './node_modules/playwright/index.mjs';
import http from 'node:http';
import { readFile } from 'node:fs/promises';
import { join, extname } from 'node:path';

const root = 'dist/frontend';
const basePath = '/api/runtime/components/assets/TESTDIGEST/';
const mime = { '.html':'text/html', '.js':'text/javascript', '.wasm':'application/wasm', '.css':'text/css', '.ttf':'font/ttf', '.woff2':'font/woff2', '.woff':'font/woff', '.svg':'image/svg+xml', '.png':'image/png' };

const server = http.createServer(async (req, res) => {
  let url = req.url.split('?')[0];
  // 模拟宿主注入: 对 index.html 注入 <base> + CSP meta
  if (url === '/' || url === basePath || url === basePath+'index.html') {
    let html = await readFile(join(root,'index.html'),'utf8');
    const csp = `sandbox allow-scripts allow-forms; default-src 'none'; script-src 'unsafe-inline' 'unsafe-eval' 'wasm-unsafe-eval' blob: http://localhost:PORT${basePath}; connect-src http://localhost:PORT${basePath} blob:; style-src 'unsafe-inline' blob: http://localhost:PORT${basePath}; img-src data: blob: http://localhost:PORT${basePath}; font-src data: http://localhost:PORT${basePath}; object-src 'none'; frame-src 'none'; worker-src blob:; base-uri http://localhost:PORT${basePath}`;
    html = html.replace('<head>', `<head><base href="${basePath}">`);
    res.setHeader('content-type','text/html');
    res.end(html); return;
  }
  // 组件 asset 根
  if (url.startsWith(basePath)) {
    const rel = url.slice(basePath.length) || 'index.html';
    try {
      const data = await readFile(join(root, rel));
      res.setHeader('content-type', mime[extname(rel)] || 'application/octet-stream');
      res.end(data); return;
    } catch { res.statusCode=404; res.end('nf'); return; }
  }
  res.statusCode=404; res.end('outside:'+url);
});
await new Promise(r=>server.listen(0,r));
const port = server.address().port;

const b = await chromium.launch({ headless:true, executablePath:'/Users/zjarlin/Library/Caches/ms-playwright/chromium-1243/chrome-mac-arm64/Google Chrome for Testing.app/Contents/MacOS/Google Chrome for Testing' });
const pg = await b.newPage();
const errs=[], blocked=[];
pg.on('console', m=>{ if(['error','warning'].includes(m.type())) errs.push(m.type()+': '+m.text()); });
pg.on('pageerror', e=>errs.push('pageerror: '+e.message));
pg.on('requestfailed', r=>blocked.push('FAILED '+r.url()));
await pg.goto(`http://localhost:${port}${basePath}index.html`, {waitUntil:'load', timeout:20000});
await pg.waitForTimeout(6000);
const body = await pg.evaluate(()=>({ text:(document.body?.innerText||'').slice(0,400), htmlLen:document.body?document.body.innerHTML.length:-1 }));
console.log('BODY:', JSON.stringify(body));
console.log('ERRS:', JSON.stringify(errs.slice(0,12),null,1));
console.log('BLOCKED:', JSON.stringify(blocked.slice(0,12),null,1));
await b.close(); server.close();
