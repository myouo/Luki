import { spawn } from 'node:child_process';
import { mkdir, mkdtemp, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

const APP_PORT = 5177;
const DEBUG_PORT = 9333;
const APP_URL = `http://127.0.0.1:${APP_PORT}/`;
const DEBUG_URL = `http://127.0.0.1:${DEBUG_PORT}`;
const SCREENSHOT_PATH = 'test-results/e2e-smoke.png';

let vite;
let chrome;
let chromeProfile;

async function main() {
try {
  await mkdir('test-results', { recursive: true });
  vite = spawn(
    'pnpm',
    ['exec', 'vite', '--host', '127.0.0.1', '--port', String(APP_PORT), '--strictPort'],
    { stdio: ['ignore', 'pipe', 'pipe'] }
  );
  await waitForHttp(APP_URL, 15_000);

  chromeProfile = await mkdtemp(join(tmpdir(), 'luki-chrome-'));
  chrome = spawn(
    'google-chrome-stable',
    [
      '--headless',
      '--disable-gpu',
      '--no-sandbox',
      '--no-first-run',
      `--user-data-dir=${chromeProfile}`,
      `--remote-debugging-port=${DEBUG_PORT}`,
      '--window-size=1440,1000',
      APP_URL
    ],
    { stdio: ['ignore', 'pipe', 'pipe'] }
  );

  const webSocketDebuggerUrl = await waitForDebuggerUrl();
  const cdp = await Cdp.connect(webSocketDebuggerUrl);
  await cdp.send('Page.enable');
  await cdp.send('Runtime.enable');
  await waitForText(cdp, '回到上次中断的位置');
  await waitForText(cdp, 'Summer Pockets');
  await clickByText(cdp, '查看回执');
  await waitForText(cdp, '游玩回执');
  await waitForText(cdp, '计时需要确认');
  await clickByText(cdp, '确认计入');
  await waitForText(cdp, '已确认');

  await clickNav(cdp, '整理');
  await waitForText(cdp, '候选项');
  await clickByText(cdp, '批量导入全部');
  await waitForText(cdp, '批量导入候选');
  await waitForText(cdp, '创建快照并导入');
  await clickByText(cdp, '取消');
  await waitForNoText(cdp, '批量导入候选');

  await clickByText(cdp, '批量导入全部');
  await waitForText(cdp, '批量导入候选');
  await clickByText(cdp, '创建快照并导入');
  await waitForText(cdp, '操作完成');
  await waitForText(cdp, '已批量导入');

  await clickNav(cdp, '书架');
  await waitForText(cdp, 'Summer Pockets');
  await waitFor(cdp, "Boolean(document.querySelector('.virtual-shelf'))", 'virtual shelf');
  await clickByText(cdp, '隐私模式');
  await waitForText(cdp, '隐私作品');
  await waitFor(cdp, "document.querySelectorAll('.cover.sensitive, .mini-cover .sensitive').length > 0", 'sensitive cover masking');
  await clickByText(cdp, '关闭隐私');
  await waitForText(cdp, 'Summer Pockets');
  const renderedGridCards = await cdp.evaluate("document.querySelectorAll('.work-card').length");
  if (renderedGridCards >= 90) {
    throw new Error(`Virtual shelf rendered too many grid cards: ${renderedGridCards}`);
  }
  await cdp.evaluate(`
    (() => {
    const shelf = document.querySelector('.virtual-shelf');
    if (!shelf) throw new Error('virtual shelf missing');
    shelf.scrollTop = shelf.scrollHeight;
    shelf.dispatchEvent(new Event('scroll', { bubbles: true }));
    })()
  `);
  await waitForText(cdp, 'Demo VN 180');
  await cdp.evaluate(`
    (() => {
    const shelf = document.querySelector('.virtual-shelf');
    if (!shelf) throw new Error('virtual shelf missing');
    shelf.scrollTop = 0;
    shelf.dispatchEvent(new Event('scroll', { bubbles: true }));
    })()
  `);
  await waitForText(cdp, 'Summer Pockets');
  await clickNav(cdp, '书架');
  await waitFor(cdp, "Boolean(document.querySelector('.virtual-shelf'))", 'virtual shelf before list mode');
  await clickByText(cdp, '列表', '.segmented-control button');
  await waitFor(cdp, "Boolean(document.querySelector('.work-list-row'))", 'compact shelf list');
  await clickByText(cdp, '网格', '.segmented-control button');
  await cdp.evaluate(`
    (() => {
      const shelf = document.querySelector('.virtual-shelf');
      if (shelf) {
        shelf.scrollTop = 0;
        shelf.dispatchEvent(new Event('scroll', { bubbles: true }));
      }
    })()
  `);
  await waitFor(cdp, "Boolean(document.querySelector('.virtual-shelf .work-card'))", 'shelf grid');
  await cdp.evaluate(`
    const card = document.querySelector('.work-card');
    if (!card) throw new Error('work card missing');
    card.click();
  `);
  await waitForText(cdp, '启动方式');
  await clickByText(cdp, '保存启动方式');
  await waitForText(cdp, '默认启动方式已更新');
  await waitForText(cdp, '封面路径');
  await cdp.evaluate(`
    const input = Array.from(document.querySelectorAll('input'))
      .find((node) => node.placeholder === '/path/to/cover.jpg');
    if (!input) throw new Error('cover path input missing');
    input.value = '/tmp/luki-e2e-cover.jpg';
    input.dispatchEvent(new Event('input', { bubbles: true }));
  `);
  await clickByText(cdp, '保存资料');
  await waitForText(cdp, '基础资料已保存');
  await waitForText(cdp, '笔记');
  await waitFor(cdp, "Boolean(document.querySelector('textarea'))", 'note textarea');
  await cdp.evaluate(`
    const textarea = document.querySelector('textarea');
    if (!textarea) throw new Error('note textarea missing');
    textarea.value = 'E2E 记录一条作品笔记';
    textarea.dispatchEvent(new Event('input', { bubbles: true }));
  `);
  await clickByText(cdp, '写入时间线');
  await waitForText(cdp, '笔记已写入作品时间线');
  await waitForText(cdp, '存档 · 分歧前');
  await clickNav(cdp, '档案');
  await waitForText(cdp, '基础资料');
  await waitForText(cdp, '存档时间线');
  await clickByText(cdp, '锁定');
  await waitForText(cdp, '存档快照已锁定');
  await waitForText(cdp, '操作历史');
  await waitForText(cdp, '更新默认启动方式');
  await waitForText(cdp, '创建笔记');
  await clickNav(cdp, '设置');
  await waitForText(cdp, '最近');
  await clickByText(cdp, '检查路径健康');
  await waitForText(cdp, '可用安装');
  await waitForText(cdp, '检查路径健康');
  await waitForText(cdp, '更新存档快照锁定');
  await waitForText(cdp, '批量导入作品');
  await clickNav(cdp, '档案');
  await waitForText(cdp, '基础资料');
  await clickByText(cdp, '移出书架');
  await waitForText(cdp, '原始文件不会被删除');
  await clickByText(cdp, '取消');
  await waitForNoText(cdp, '原始文件不会被删除');

  const screenshot = await cdp.send('Page.captureScreenshot', { format: 'png' });
  await writeFile(SCREENSHOT_PATH, Buffer.from(screenshot.data, 'base64'));
  await cdp.close();
  console.log(`E2E smoke passed. Screenshot: ${SCREENSHOT_PATH}`);
} finally {
  if (chrome) chrome.kill('SIGTERM');
  if (vite) vite.kill('SIGTERM');
  await sleep(500);
  if (chromeProfile) {
    await rm(chromeProfile, { recursive: true, force: true, maxRetries: 5, retryDelay: 100 });
  }
}
}

async function waitForHttp(url, timeoutMs) {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    try {
      const response = await fetch(url);
      if (response.ok) return;
    } catch {
      await sleep(150);
    }
  }
  throw new Error(`Timed out waiting for ${url}`);
}

async function waitForDebuggerUrl() {
  const deadline = Date.now() + 15_000;
  while (Date.now() < deadline) {
    try {
      const response = await fetch(`${DEBUG_URL}/json`);
      if (response.ok) {
        const targets = await response.json();
        const page =
          targets.find(
            (target) =>
              target.type === 'page' &&
              target.url?.startsWith(APP_URL) &&
              target.webSocketDebuggerUrl
          ) ??
          targets.find((target) => target.type === 'page' && target.webSocketDebuggerUrl);
        if (page) return page.webSocketDebuggerUrl;
      }
    } catch {
      await sleep(150);
    }
  }
  throw new Error('Timed out waiting for Chrome debugger URL');
}

async function waitForText(cdp, text) {
  await waitFor(
    cdp,
    `(document.body?.innerText ?? '').includes(${JSON.stringify(text)})`,
    `text ${text}`
  );
}

async function waitForNoText(cdp, text) {
  await waitFor(
    cdp,
    `!(document.body?.innerText ?? '').includes(${JSON.stringify(text)})`,
    `no text ${text}`
  );
}

async function waitFor(cdp, expression, label) {
  const deadline = Date.now() + 10_000;
  let lastError;
  while (Date.now() < deadline) {
    try {
      const result = await cdp.evaluate(`Boolean(${expression})`);
      if (result === true) return;
    } catch (error) {
      lastError = error;
    }
    await sleep(100);
  }
  if (lastError) console.error(`Last wait error for ${label}: ${lastError.message}`);
  const bodyText = await cdp.evaluate("document.body?.innerText ?? ''");
  console.error(bodyText);
  throw new Error(`Timed out waiting for ${label}`);
}

async function clickByText(cdp, text, selector = 'button') {
  const expression = `
    (() => {
      const target = Array.from(document.querySelectorAll(${JSON.stringify(selector)}))
        .find((node) => node.textContent.trim().includes(${JSON.stringify(text)}));
      if (!target) return false;
      target.click();
      return true;
    })()
  `;
  const clicked = await cdp.evaluate(expression);
  if (clicked !== true) throw new Error(`Could not click ${text}`);
}

async function clickNav(cdp, label) {
  const expression = `
    (() => {
      const target = Array.from(document.querySelectorAll('nav button'))
        .find((node) => node.querySelector('span')?.textContent.trim() === ${JSON.stringify(label)});
      if (!target) return false;
      target.click();
      return true;
    })()
  `;
  const clicked = await cdp.evaluate(expression);
  if (clicked !== true) throw new Error(`Could not click nav ${label}`);
}

function sleep(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

class Cdp {
  static connect(url) {
    return new Promise((resolve, reject) => {
      const socket = new WebSocket(url);
      const cdp = new Cdp(socket);
      socket.addEventListener('open', () => resolve(cdp), { once: true });
      socket.addEventListener('error', reject, { once: true });
    });
  }

  constructor(socket) {
    this.socket = socket;
    this.nextId = 1;
    this.pending = new Map();
    socket.addEventListener('message', (event) => {
      const message = JSON.parse(event.data);
      if (!message.id) return;
      const request = this.pending.get(message.id);
      if (!request) return;
      this.pending.delete(message.id);
      if (message.error) {
        request.reject(new Error(message.error.message));
      } else {
        request.resolve(message.result);
      }
    });
  }

  send(method, params = {}) {
    const id = this.nextId++;
    const payload = JSON.stringify({ id, method, params });
    return new Promise((resolve, reject) => {
      this.pending.set(id, { resolve, reject });
      this.socket.send(payload);
    });
  }

  async evaluate(expression) {
    const result = await this.send('Runtime.evaluate', {
      expression,
      awaitPromise: true,
      returnByValue: true
    });
    if (result.exceptionDetails) {
      throw new Error(
        result.exceptionDetails.exception?.description ??
          result.exceptionDetails.text ??
          'Runtime evaluation failed'
      );
    }
    return result.result.value;
  }

  close() {
    this.socket.close();
  }
}

await main();
