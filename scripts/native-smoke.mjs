// Native WebView2 IPC + UI checks. Only this test process enables loopback CDP.
import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import fs from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import { chromium, expect } from '@playwright/test';

if (process.platform !== 'win32') throw new Error('Native smoke requires Windows');
const binary = path.resolve('target/release/bebekoncode-desktop.exe');
await fs.access(binary);
const fixture = await fs.mkdtemp(path.join(os.tmpdir(), 'bebekon-native-'));
const project = path.join(fixture, 'проект с пробелами');
await fs.mkdir(project);
await fs.writeFile(path.join(project, 'hello.txt'), 'Hello from the real disk.\r\n');
await fs.mkdir(path.join(project, '.git'));
const port = 9237;
const child = spawn(binary, ['--data-dir', path.join(fixture, 'data')], {
  windowsHide: true,
  stdio: 'ignore',
  env: {
    ...process.env,
    WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: `--remote-debugging-port=${port} --remote-debugging-address=127.0.0.1`,
  },
});
let browser;
let exited = false;
child.once('exit', () => {
  exited = true;
});
const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));
try {
  let connected = false;
  for (let i = 0; i < 100; i++) {
    if (exited) throw new Error('Desktop exited before WebView2 connected');
    try {
      const response = await fetch(`http://127.0.0.1:${port}/json/version`);
      if (response.ok) {
        connected = true;
        break;
      }
    } catch {
      /* WebView2 is still starting. */
    }
    await sleep(200);
  }
  assert(connected, 'WebView2 debug connection unavailable');
  browser = await chromium.connectOverCDP(`http://127.0.0.1:${port}`);
  let page;
  for (let i = 0; i < 50; i++) {
    page = browser
      .contexts()
      .flatMap((context) => context.pages())
      .find((page) => page.url().includes('tauri.localhost'));
    if (page) break;
    await sleep(200);
  }
  assert(page, 'Native packaged page unavailable');
  const errors = [];
  page.on('pageerror', (error) => errors.push(error.message));
  await page.getByText('BebekonCode', { exact: true }).first().waitFor();
  // Seed the isolated profile through the real Tauri bridge, not an in-memory adapter.
  const workspace = await page.evaluate(
    (root) => window.__TAURI_INTERNALS__.invoke('add_workspace', { root }),
    project,
  );
  await page.reload();
  await page.getByRole('button', { name: 'Файлы проекта', exact: true }).click();
  await page.getByRole('button', { name: 'hello.txt', exact: true }).click();
  const editor = page.getByRole('textbox', { name: 'Содержимое файла' });
  assert.equal(await editor.inputValue(), 'Hello from the real disk.\n');
  await editor.fill('Edited in the native desktop.\nснеговик');
  // This drives a real desktop window; stray keystrokes from the user would land here.
  assert.equal(
    await editor.inputValue(),
    'Edited in the native desktop.\nснеговик',
    'Editor content changed before save: keyboard input reached the test window',
  );
  await editor.press('Control+s');
  await page.getByText('Сохранено на диске.', { exact: true }).waitFor();
  assert.equal(
    await fs.readFile(path.join(project, 'hello.txt'), 'utf8'),
    'Edited in the native desktop.\r\nснеговик',
  );

  // A change by another process must survive a rejected editor save.
  await editor.fill('Must not overwrite external edits');
  await fs.writeFile(path.join(project, 'hello.txt'), 'External writer');
  await editor.press('Control+s');
  await page.getByText(/Файл изменился на диске/).waitFor();
  assert.equal(await fs.readFile(path.join(project, 'hello.txt'), 'utf8'), 'External writer');
  await page.getByRole('button', { name: 'Перечитать с диска', exact: true }).click();
  await page.getByRole('button', { name: 'Не сохранять', exact: true }).click();
  await page.waitForFunction(
    () => document.querySelector('[aria-label="Содержимое файла"]')?.value === 'External writer',
  );

  await page.getByRole('button', { name: 'Новый файл', exact: true }).click();
  await page.getByRole('textbox', { name: 'Имя файла' }).fill('new.txt');
  await page.getByRole('button', { name: 'Подтвердить', exact: true }).click();
  await page.getByRole('button', { name: 'new.txt', exact: true }).click();
  await page.getByRole('button', { name: 'Переименовать / переместить', exact: true }).click();
  await page.getByRole('textbox', { name: 'Имя файла' }).fill('renamed.txt');
  await page.getByRole('button', { name: 'Подтвердить', exact: true }).click();
  await page.getByRole('button', { name: 'renamed.txt', exact: true }).click();
  await fs.access(path.join(project, 'renamed.txt'));
  await page.getByRole('button', { name: 'Удалить…', exact: true }).click();
  await page.getByRole('button', { name: 'Удалить безвозвратно', exact: true }).click();
  await page.getByRole('button', { name: 'renamed.txt', exact: true }).waitFor({ state: 'hidden' });
  await assert.rejects(fs.access(path.join(project, 'renamed.txt')));

  await page.getByRole('button', { name: 'Новая папка', exact: true }).click();
  await page.getByRole('textbox', { name: 'Имя файла' }).fill('новая папка');
  await page.getByRole('button', { name: 'Подтвердить', exact: true }).click();
  await page.getByRole('button', { name: 'новая папка', exact: true }).click();
  await page.getByRole('button', { name: 'На уровень выше', exact: true }).click();
  assert((await fs.stat(path.join(project, 'новая папка'))).isDirectory());
  assert(await page.getByRole('button', { name: '.git', exact: true }).isDisabled());
  const escaped = await page.evaluate(async (workspaceId) => {
    try {
      await window.__TAURI_INTERNALS__.invoke('file_operation', {
        workspaceId,
        operation: { kind: 'create', path: '../outside.txt', directory: false },
      });
      return false;
    } catch {
      return true;
    }
  }, workspace.id);
  assert(escaped, 'Traversal accepted by native IPC');
  await editor.waitFor({ state: 'hidden' });
  await page.getByRole('button', { name: 'hello.txt', exact: true }).click();
  await editor.fill('Unsaved changes');
  await page.getByRole('dialog').getByRole('button', { name: 'Закрыть окно', exact: true }).click();
  await page.getByText(/Не сохранять изменения в файле/).waitFor();
  await page.getByRole('button', { name: 'Продолжить редактирование', exact: true }).click();
  assert.equal(await editor.inputValue(), 'Unsaved changes');
  const windowCloser = spawn(
    'powershell.exe',
    ['-NoProfile', '-Command', `(Get-Process -Id ${child.pid}).CloseMainWindow() | Out-Null`],
    { windowsHide: true, stdio: 'ignore' },
  );
  await new Promise((resolve) => windowCloser.once('exit', resolve));
  await page.getByText(/Не сохранять изменения в файле/).waitFor();
  assert(!exited, 'Native window closed despite unsaved edits');
  await page.getByRole('button', { name: 'Продолжить редактирование', exact: true }).click();
  // Global shortcuts must not replace the editor and silently discard its buffer.
  await editor.press('Control+k');
  assert.equal(await editor.inputValue(), 'Unsaved changes');
  await page.getByRole('dialog').getByRole('button', { name: 'Закрыть окно', exact: true }).click();
  await page.getByRole('button', { name: 'Не сохранять', exact: true }).click();
  await page.getByRole('button', { name: 'Файлы проекта', exact: true }).click();
  await page.getByRole('button', { name: 'hello.txt', exact: true }).click();
  await fs.mkdir('test-results', { recursive: true });
  await page.screenshot({ path: 'test-results/native-files.png' });
  await page.getByRole('dialog').getByRole('button', { name: 'Закрыть окно', exact: true }).click();
  // The published SQL seed/defaults stay unchanged; old built-in labels localize on display.
  await page.evaluate(
    (workspaceId) =>
      window.__TAURI_INTERNALS__.invoke('create_session', {
        input: {
          workspace_id: workspaceId,
          provider: 'mock',
          account_profile_id: 'mock-local',
          model: 'mock-stream-v1',
          permission_profile: 'standard',
        },
      }),
    workspace.id,
  );
  await page.reload();
  // Legacy persisted title 'New session' is presented in Russian without a DB rewrite.
  await page.locator('.session-title', { hasText: /^Новый чат$/ }).waitFor();
  await page.getByText('Ожидает задачи', { exact: true }).first().waitFor();
  assert.equal(await page.getByText('Local demo', { exact: true }).count(), 0);
  const composer = page.getByRole('textbox', { name: 'Сообщение агенту' });
  await composer.fill('Проверка русского интерфейса');
  await composer.press('Control+Enter');
  await page.getByText('Это ответ локального демо', { exact: false }).waitFor();
  await page.getByRole('button', { name: 'Остановить агента', exact: true }).click();
  await page.getByText(/^Остановлено вами/).waitFor();
  await page.screenshot({ path: 'test-results/native-russian.png' });
  // New chat commands use real Rust + SQLite in this isolated profile.
  const agent = {
    provider: 'mock',
    account_profile_id: 'mock-local',
    model: 'mock-stream-v1',
    reasoning_effort: null,
    permission_profile: 'standard',
    tools: {},
    role: '',
  };
  const chat = await page.evaluate(
    (agent) =>
      window.__TAURI_INTERNALS__.invoke('create_chat', {
        input: { workspace_id: null, mode: 'auto', agents: [agent] },
      }),
    agent,
  );
  assert.equal(chat.workspace_id, 'chat-scratch');
  await page.evaluate(
    ({ id, agent }) =>
      window.__TAURI_INTERNALS__.invoke('configure_session', {
        sessionId: id,
        config: { ...agent, permission_profile: 'read_only' },
      }),
    { id: chat.id, agent },
  );
  await page.evaluate(
    (id) =>
      window.__TAURI_INTERNALS__.invoke('send_message', {
        sessionId: id,
        prompt: 'Демо: проверить два независимых контекста',
      }),
    chat.id,
  );
  await expect
    .poll(
      () =>
        page.evaluate(async (id) => {
          const snapshot = await window.__TAURI_INTERNALS__.invoke('snapshot');
          return snapshot.sessions.find((s) => s.id === id)?.status;
        }, chat.id),
      { timeout: 30_000 },
    )
    .toBe('completed');
  const tasks = await page.evaluate(
    (id) =>
      window.__TAURI_INTERNALS__
        .invoke('snapshot')
        .then((snapshot) => snapshot.sessions.filter((s) => s.parent_session_id === id)),
    chat.id,
  );
  assert.equal(tasks.length, 2);
  assert(
    tasks.every(
      (s) =>
        s.permission_profile === 'read_only' && s.provider !== 'openai' && s.status === 'completed',
    ),
  );
  await page.evaluate(
    ({ id, agent }) =>
      window.__TAURI_INTERNALS__.invoke('handoff_session', { sessionId: id, config: agent }),
    { id: chat.id, agent },
  );
  await expect
    .poll(
      () =>
        page.evaluate(async (id) => {
          const snapshot = await window.__TAURI_INTERNALS__.invoke('snapshot');
          const chat = snapshot.sessions.find((s) => s.id === id);
          return chat?.status === 'completed' && chat.context_summary.length > 0;
        }, chat.id),
      { timeout: 20_000 },
    )
    .toBe(true);
  await page.reload();
  await page
    .getByText('Демо: проверить два независимых контекста', { exact: true })
    .first()
    .waitFor();
  await page.screenshot({ path: 'test-results/native-chats.png' });
  console.log(
    'PASS: native chat without project, configuration, parallel demo contexts, handoff and persisted history',
  );
  if (process.argv.includes('--updates')) {
    await page.getByRole('button', { name: 'Настройки', exact: true }).click();
    await page
      .getByRole('dialog')
      .getByRole('button', { name: 'О программе и обновления' })
      .click();
    await page.getByRole('button', { name: 'Проверить обновления', exact: true }).click();
    await page.getByText(/Установлена актуальная версия/).waitFor({ timeout: 45_000 });
    assert.equal(
      await page.getByRole('button', { name: 'Установить и перезапустить', exact: true }).count(),
      0,
    );
    await page.screenshot({ path: 'test-results/native-update-current.png' });
    console.log('PASS: native signed updater checked the public GitHub manifest');
  }
  assert.deepEqual(errors, []);
  console.log(
    'PASS: native packaged WebView2, real IPC/disk CRUD, conflict handling, Unicode paths, traversal, dirty-navigation, native window close and branding',
  );
} finally {
  await browser?.close();
  if (!exited) {
    const closer = spawn(
      'powershell.exe',
      ['-NoProfile', '-Command', `(Get-Process -Id ${child.pid}).CloseMainWindow() | Out-Null`],
      { windowsHide: true, stdio: 'ignore' },
    );
    await new Promise((resolve) => closer.once('exit', resolve));
    for (let i = 0; i < 50 && !exited; i++) await sleep(100);
    if (!exited) child.kill();
  }
  // Intentionally preserve the isolated fixture for inspection, never touch default app data.
  console.log(`Native smoke profile: ${fixture}`);
}
