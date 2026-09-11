import assert from 'node:assert/strict';
import { mkdir, writeFile } from 'node:fs/promises';
import { chromium } from 'playwright';
import { PNG } from 'pngjs';

const url = process.env.AIO_PREVIEW_URL || 'http://127.0.0.1:4191/';
const output = 'test-results';
await mkdir(output, { recursive: true });
const browser = await chromium.launch({ channel: 'chrome', headless: true });
const reports = [];
const pause = ms => new Promise(resolve => setTimeout(resolve, ms));
async function click(locator) { await locator.waitFor(); await pause(250); await locator.click({ force: true }); await pause(150); }
function responseFor(page, method, path) {
  const promise = page.waitForResponse(response => {
    if (new URL(response.url()).pathname !== '/invoke') return false;
    const input = response.request().postDataJSON();
    return input?.method === method && input.path === path;
  }, { timeout: 30000 });
  promise.catch(() => {}); return promise;
}
async function payload(response, status = 200) {
  assert.equal(response.status(), 200);
  const value = await response.json();
  assert.equal(value.status, status, Buffer.from(value.body).toString());
  return status === 204 ? null : JSON.parse(Buffer.from(value.body).toString());
}
async function reload(page) {
  const pending = responseFor(page, 'GET', '/graph');
  await page.goto(url); const graph = await payload(await pending); await pause(700); return graph;
}
async function typeInto(page, locator, value) {
  await click(locator); await page.keyboard.press('Meta+A'); await page.keyboard.insertText(value); await pause(150);
}
async function select(page, frame, title, id) {
  await click(frame.getByRole('button', { name: '列表', exact: true }));
  const node = responseFor(page, 'GET', `/nodes/${id}`);
  const edges = responseFor(page, 'GET', `/nodes/${id}/edges`);
  await click(frame.getByText(title, { exact: false }).first());
  await payload(await node); await payload(await edges); await pause(300);
}
async function sdk(frame, method, path, body) {
  return frame.locator('body').evaluate((_, input) => window.aioPlugin.json(input.method, input.path, input.body), { method, path, body });
}
try {
  for (const [name, viewport] of [['desktop', { width: 1440, height: 960 }], ['mobile', { width: 390, height: 844 }]]) {
    const context = await browser.newContext({ viewport });
    const page = await context.newPage();
    const errors = [], requests = [], external = [];
    page.on('pageerror', error => errors.push(error.message));
    page.on('console', message => { if (message.type() === 'error') errors.push(message.text()); });
    page.on('request', request => {
      if (new URL(request.url()).pathname === '/invoke') requests.push(request.postDataJSON());
      if (!request.url().startsWith(new URL(url).origin)) external.push(request.url());
    });
    const frame = page.frameLocator('iframe');
    const cleanup = [];
    try {
      const original = await reload(page);
      assert(original.nodes.length > 1, 'Run preview with --demo');
      const canvas = frame.locator('canvas').first();
      await canvas.waitFor();
      const points = {};
      for (const label of ['列表', '图谱', '新建记忆', '导入来源', '刷新']) {
        const bounds = await frame.getByRole('button', { name: label, exact: true }).boundingBox();
        assert(bounds, `Missing control ${label}`);
        points[label] = { x: bounds.x + bounds.width / 2, y: bounds.y + bounds.height / 2 };
      }
      const pointer = async label => { await page.mouse.click(points[label].x, points[label].y); await pause(300); };
      await click(frame.getByRole('button', { name: '暂停布局', exact: true }));
      await pause(300);
      const before = PNG.sync.read(await page.screenshot());
      const colors = new Set();
      for (let i = 0; i < before.data.length; i += 4) colors.add(before.data.readUInt32BE(i));
      assert(colors.size > 100, 'Compose canvas is blank');
      const width = name === 'desktop' ? viewport.width - 320 : viewport.width;
      let point;
      for (let y = 250; y < viewport.height - 90 && !point; y++) for (let x = 30; x < width - 40; x++) {
        const offset = (y * before.width + x) * 4;
        if (before.data[offset] === 71 && before.data[offset + 1] === 111 && before.data[offset + 2] === 189) { point = { x, y: y + 20 }; break; }
      }
      assert(point, 'Missing rendered graph node');
      const count = requests.length;
      await context.setOffline(true);
      await page.mouse.move(point.x, point.y); await page.mouse.down();
      await page.mouse.move(point.x + 50, point.y + 35, { steps: 12 }); await page.mouse.up(); await pause(300);
      const after = PNG.sync.read(await page.screenshot());
      let changed = 0;
      for (let y = 210; y < viewport.height - 60; y++) for (let x = 0; x < width; x++) {
        const i = (y * before.width + x) * 4;
        if (before.data.readUInt32BE(i) !== after.data.readUInt32BE(i)) changed++;
      }
      assert(changed > 200, 'Graph did not move');
      await pointer('列表');
      await pointer('图谱');
      assert.equal(requests.length, count, 'Local graph interaction issued a request');
      await context.setOffline(false);
      await page.screenshot({ path: `${output}/${name}-graph.png` });

      const title = `Browser memory ${name} ${Date.now()}`;
      await pointer('新建记忆');
      let fields = frame.getByRole('textbox').filter({ visible: true });
      await typeInto(page, fields.nth(0), title);
      await typeInto(page, fields.nth(1), 'Markdown memory\n\nPersistent body.');
      await page.screenshot({ path: `${output}/${name}-editor.png` });
      const creation = responseFor(page, 'POST', '/nodes');
      await click(frame.getByRole('button', { name: '保存', exact: true }));
      const node = await payload(await creation, 201); cleanup.push(node.id);
      assert.equal(node.title, title); assert.equal(node.content, 'Markdown memory\n\nPersistent body.');
      await pause(500);
      const persisted = await reload(page);
      assert(persisted.nodes.some(value => value.id === node.id), 'Refresh lost persisted node');
      await select(page, frame, title, node.id);
      await click(frame.getByRole('button', { name: '编辑记忆', exact: true }));
      fields = frame.getByRole('textbox').filter({ visible: true });
      await typeInto(page, fields.nth(1), 'Updated from Compose');
      const update = responseFor(page, 'PUT', `/nodes/${node.id}`);
      await click(frame.getByRole('button', { name: '保存', exact: true }));
      assert.equal((await payload(await update)).version, 2);
      await pause(300); await reload(page); await select(page, frame, title, node.id);
      await sdk(frame, 'PUT', `/nodes/${node.id}`, { title, content: 'Updated in another session', version: 2 });
      if (name === 'mobile') {
        await click(frame.getByRole('button', { name: '关闭详情', exact: true }));
        const refreshedGraph = responseFor(page, 'GET', '/graph');
        await pointer('刷新'); await page.mouse.move(16, 16);
        assert.equal((await payload(await refreshedGraph)).nodes.find(value => value.id === node.id).version, 3);
        await pause(300); await reload(page); await select(page, frame, title, node.id);
      } else {
        const refreshedNode = responseFor(page, 'GET', `/nodes/${node.id}`);
        await pointer('刷新'); await page.mouse.move(16, 16);
        assert.equal((await payload(await refreshedNode)).version, 3);
      }
      await pause(300);
      await click(frame.getByRole('button', { name: '删除记忆', exact: true }));
      await click(frame.getByRole('button', { name: '取消', exact: true }));
      assert.equal((await sdk(frame, 'GET', `/nodes/${node.id}`)).content, 'Updated in another session');
      await reload(page); await select(page, frame, title, node.id);
      await click(frame.getByRole('button', { name: '删除记忆', exact: true }));
      const deletion = responseFor(page, 'DELETE', `/nodes/${node.id}`);
      await click(frame.getByRole('button', { name: '确认删除', exact: true }));
      await payload(await deletion, 204); cleanup.splice(cleanup.indexOf(node.id), 1);
      await pause(300); await reload(page);

      if (name === 'desktop') {
        const filename = `source-${Date.now()}`;
        const concept = `Concept-${Date.now()}`;
        await click(frame.getByRole('button', { name: '导入来源', exact: true }));
        const chooser = page.waitForEvent('filechooser');
        await click(frame.getByRole('button', { name: '选择 Markdown / 文本文件', exact: true }));
        await (await chooser).setFiles({ name: `${filename}.md`, mimeType: 'text/markdown', buffer: Buffer.from(`# Source\n\n[[${concept}]] retains evidence.`) });
        await pause(200);
        const imported = responseFor(page, 'POST', '/import');
        await click(frame.getByRole('button', { name: '导入', exact: true }));
        const result = await payload(await imported, 201); cleanup.push(result.source.id);
        assert.equal(result.linkedNodes, 1);
        await pause(300); const graph = await reload(page);
        cleanup.push(graph.nodes.find(n => n.title === concept).id);
        await select(page, frame, filename, result.source.id);
        const exported = responseFor(page, 'POST', '/context');
        await click(frame.getByRole('button', { name: '导出上下文', exact: true }).last());
        const text = await payload(await exported);
        assert(text.markdown.includes(concept)); assert(text.nodeIds.includes(result.source.id));
        await page.screenshot({ path: `${output}/desktop-context.png` });
        await click(frame.getByRole('button', { name: '关闭', exact: true }));
      }
      await reload(page);
      assert(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth));
      assert(await frame.locator('body').evaluate(() => document.documentElement.scrollWidth <= innerWidth));
      const isolation = await frame.locator('body').evaluate(() => {
        let parentBlocked = false, cookieBlocked = false;
        try { void parent.document.body; } catch { parentBlocked = true; }
        try { void document.cookie; } catch { cookieBlocked = true; }
        return { parentBlocked, cookieBlocked };
      });
      assert.deepEqual(isolation, { parentBlocked: true, cookieBlocked: true });
      assert.equal((await context.request.post(new URL('/invoke', url).href, { data: { method: 'GET', path: '/graph', body: [] } })).status(), 403);
      assert.deepEqual(errors, []); assert.deepEqual(external, [], 'Frontend requested an external asset');
      reports.push({ name, canvasColors: colors.size, dragChangedPixels: changed, localRequests: 0, crud: true, refreshPersistence: true, isolation, consoleErrors: errors.length });
    } catch (error) {
      await page.screenshot({ path: `${output}/${name}-failure.png` }); console.error(errors); throw error;
    } finally {
      await context.setOffline(false);
      for (const id of cleanup) await sdk(frame, 'DELETE', `/nodes/${id}`).catch(() => {});
      await context.close();
    }
  }
  await writeFile(`${output}/browser-report.json`, JSON.stringify(reports, null, 2));
  console.log(JSON.stringify(reports, null, 2));
} finally { await browser.close(); }
