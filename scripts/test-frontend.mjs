import { chromium } from "playwright";
import { expect } from "playwright/test";
import { mkdir } from "node:fs/promises";
import { startFixture } from "./frontend-fixture.mjs";

const { server, url } = await startFixture();
const browser = await chromium.launch({ headless: true, executablePath: process.env.PLAYWRIGHT_CHROMIUM_EXECUTABLE_PATH || undefined });
await mkdir("test-results", { recursive: true });
const errors = [];
const page = await browser.newPage({ viewport: { width: 1440, height: 1000 } });
page.on("pageerror", (error) => errors.push(error.message));
page.on("console", (message) => { if (message.type() === "error") errors.push(`${message.text()} ${message.location().url}`); });
let frame;
async function open(query = "") {
  await page.goto(url + query, { waitUntil: "networkidle" });
  frame = page.frames().find((item) => item.url().includes("TESTDIGEST"));
  await expect(frame.getByRole("heading", { name: "记忆", exact: true })).toBeVisible();
  await expect(frame.locator(".memory-note")).toHaveCount(24);
}
async function geometry() {
  if (await frame.getByRole("dialog").count()) {
    await expect(frame.locator(".dx-dialog-backdrop")).toHaveCSS("opacity", "1");
    await expect(frame.getByRole("dialog")).not.toHaveCSS("background-color", "rgba(0, 0, 0, 0)");
  }
  const problems = await frame.evaluate(() => {
    const issues = [];
    if (document.documentElement.scrollWidth > innerWidth + 1) issues.push("horizontal overflow");
    for (const element of document.querySelectorAll("button,.memory-note__status,.memory-note__meta time")) {
      const rect = element.getBoundingClientRect();
      if (rect.width && (element.scrollWidth > element.clientWidth + 2 || element.scrollHeight > element.clientHeight + 2)) issues.push(`clipped: ${element.textContent || element.getAttribute("aria-label")}`);
    }
    const dialog = document.querySelector("[role=dialog]");
    if (dialog) { const rect = dialog.getBoundingClientRect(); if (rect.top < 0 || rect.bottom > innerHeight + 1 || rect.left < 0 || rect.right > innerWidth + 1) issues.push("dialog outside viewport"); }
    return issues;
  });
  expect(problems).toEqual([]);
}

try {
  await open();
  await geometry();
  await page.screenshot({ path: "test-results/memory-desktop.png" });
  await frame.getByRole("button", { name: "下一页", exact: true }).click();
  await expect(frame.locator(".memory-note")).toHaveCount(4);
  await frame.getByRole("button", { name: "上一页", exact: true }).click();
  await expect(frame.locator(".memory-note")).toHaveCount(24);
  await frame.getByRole("textbox", { name: "搜索记录" }).fill("归档 28");
  await frame.getByRole("button", { name: "搜索", exact: true }).click();
  await expect(frame.locator(".memory-note")).toHaveCount(1);
  await expect(frame.locator(".memory-note")).toContainText("归档 28");
  await frame.getByRole("textbox", { name: "搜索记录" }).fill("");
  await frame.getByRole("button", { name: "搜索", exact: true }).click();
  await frame.getByRole("combobox", { name: "整理状态" }).selectOption("pending");
  await expect(frame.locator(".memory-note")).toHaveCount(7);
  await frame.getByRole("combobox", { name: "整理状态" }).selectOption("");

  const input = frame.getByRole("textbox", { name: "随心记内容" });
  const note = "# 验收记录\n\n- 完成 CRUD\n- 检查预览\n\n| 项目 | 状态 |\n| --- | --- |\n| 排版 | 完成 |\n\n```rust\nlet done = true;\n```";
  await input.fill(note);
  await frame.getByRole("button", { name: "预览", exact: true }).click();
  await expect(frame.locator(".memory-composer__preview h1")).toHaveText("验收记录");
  await expect(frame.locator(".memory-composer__preview table")).toBeVisible();
  await expect(frame.locator(".memory-composer__preview pre")).toContainText("let done = true;");
  await frame.getByRole("button", { name: "编辑", exact: true }).click();
  await frame.getByRole("tab", { name: "图谱", exact: true }).click();
  await frame.getByRole("tab", { name: "随心记", exact: true }).click();
  await expect(input).toHaveValue(note);
  await frame.getByRole("button", { name: "知识空间", exact: true }).click();
  await frame.getByRole("option", { name: "项目笔记", exact: true }).click();
  await expect(input).toHaveValue("");
  await input.fill("项目空间的独立草稿");
  await frame.getByRole("button", { name: "知识空间", exact: true }).click();
  await frame.getByRole("option", { name: "个人记忆", exact: true }).click();
  await expect(input).toHaveValue(note);
  await frame.evaluate(() => { window.__memoryFail = "POST /capture"; });
  await frame.getByRole("button", { name: "保存随心记" }).click();
  await expect(frame.getByText("请求失败，请重试", { exact: true })).toBeVisible();
  await expect(input).toHaveValue(note);
  await input.press("Control+Enter");
  await expect(input).toHaveValue("");
  const card = frame.locator(".memory-note").filter({ hasText: "验收记录" });
  await expect(card).toBeVisible();
  const captures = await frame.evaluate(() => window.__memoryCalls.filter((call) => call.path === "/capture"));
  expect(captures).toHaveLength(2);
  expect(captures[0].body.requestId).toBe(captures[1].body.requestId);

  await card.getByRole("button", { name: "查看详情" }).click();
  await expect(frame.getByRole("dialog").locator("table")).toBeVisible();
  await geometry();
  await page.screenshot({ path: "test-results/memory-detail.png" });
  await frame.getByRole("button", { name: "编辑记录", exact: true }).last().click();
  const editor = frame.getByRole("textbox", { name: "编辑记录正文" });
  await expect(editor).toHaveValue(note);
  const revised = note + "\n\n修改已经保存。";
  await editor.fill(revised);
  await frame.evaluate(() => { window.__memoryFail = "PUT /sources/note-100"; });
  await frame.getByRole("button", { name: "保存修改" }).click();
  await expect(frame.getByText("版本已变化，请重新打开记录后合并修改", { exact: true })).toBeVisible();
  await expect(editor).toHaveValue(revised);
  await frame.getByRole("button", { name: "保存修改" }).click();
  await expect(frame.getByRole("dialog")).toHaveCount(0);
  await expect(card).toContainText("修改已经保存。");
  await card.getByRole("button", { name: "删除记录" }).click();
  await frame.getByRole("button", { name: "取消", exact: true }).click();
  await expect(card).toBeVisible();
  await card.getByRole("button", { name: "删除记录" }).click();
  await frame.evaluate(() => { window.__memoryFail = "DELETE /sources/note-100"; });
  await frame.getByRole("button", { name: "确认删除" }).click();
  await expect(frame.getByText("请求失败，请重试", { exact: true })).toBeVisible();
  await frame.getByRole("button", { name: "确认删除" }).click();
  await expect(card).toHaveCount(0);
  await expect(frame.getByRole("dialog")).toHaveCount(0);

  await frame.getByRole("button", { name: "导入资料", exact: true }).click();
  await frame.getByRole("textbox", { name: "资料正文", exact: true }).fill("导入后立即显示在随心记列表");
  await frame.getByRole("dialog").getByRole("button", { name: "保存", exact: true }).click();
  await expect(frame.getByRole("dialog")).toHaveCount(0);
  await expect(frame.locator(".memory-note").filter({ hasText: "导入后立即显示" })).toBeVisible();

  for (const width of [390, 320]) {
    await page.setViewportSize({ width, height: 844 });
    await geometry();
    await page.screenshot({ path: `test-results/memory-mobile-${width}.png` });
    await frame.locator(".memory-note").first().getByRole("button", { name: "查看详情" }).click();
    await expect(frame.getByRole("dialog").locator(".dx-markdown")).toBeVisible();
    await geometry();
    await page.screenshot({ path: `test-results/memory-mobile-detail-${width}.png` });
    await frame.getByRole("button", { name: "关闭详情" }).click();
  }
  await frame.evaluate(() => { window.__memorySources[0].text = "# 长记录\n\n" + "正文需要独立滚动，底部操作保持可见。\n\n".repeat(80); });
  await frame.locator(".memory-note").first().getByRole("button", { name: "查看详情" }).click();
  await expect(frame.getByRole("dialog").getByRole("heading", { name: "长记录", exact: true })).toBeVisible();
  await geometry();
  expect(await frame.locator(".memory-dialog__body").evaluate((body) => body.scrollHeight > body.clientHeight)).toBe(true);
  await expect(frame.getByRole("dialog").getByRole("button", { name: "编辑记录", exact: true })).toBeInViewport();
  await frame.getByRole("button", { name: "关闭详情" }).click();
  await page.setViewportSize({ width: 1440, height: 1000 });
  await open("?readonly");
  await expect(frame.getByRole("textbox", { name: "随心记内容" })).toBeDisabled();
  await expect(frame.getByRole("button", { name: "导入资料", exact: true })).toBeDisabled();
  await expect(frame.getByRole("button", { name: "编辑记录", exact: true })).toHaveCount(0);
  await expect(frame.getByRole("button", { name: "删除记录", exact: true })).toHaveCount(0);
  expect(errors).toEqual([]);
  console.log("Memory sandbox UI passed: CRUD, Markdown, drafts, retries, conflict, pagination, search, filtering, readonly, desktop/mobile geometry.");
} catch (error) {
  await page.screenshot({ path: "test-results/memory-failure.png" });
  console.error("Browser errors:", errors);
  throw error;
} finally {
  await browser.close();
  server.close();
}
