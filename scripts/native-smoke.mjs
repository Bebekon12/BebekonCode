// Native WebView2 IPC + UI checks. Only this test process enables loopback CDP.
import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import fs from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import { chromium } from '@playwright/test';

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
  await page.getByText('BebekonCode', { exact: true }).waitFor();
  // Seed the isolated profile through the real Tauri bridge, not an in-memory adapter.
  const workspace = await page.evaluate(
    (root) => window.__TAURI_INTERNALS__.invoke('add_workspace', { root }),
    project,
  );
  await page.reload();
  await page.getByRole('button', { name: 'Project files', exact: true }).click();
  await page.getByRole('button', { name: 'hello.txt', exact: true }).click();
  const editor = page.getByRole('textbox', { name: 'File content' });
  assert.equal(await editor.inputValue(), 'Hello from the real disk.\n');
  await editor.fill('Edited in the native desktop.\nснеговик');
  await editor.press('Control+s');
  await page.getByText('Saved to disk.', { exact: true }).waitFor();
  assert.equal(
    await fs.readFile(path.join(project, 'hello.txt'), 'utf8'),
    'Edited in the native desktop.\r\nснеговик',
  );

  // A change by another process must survive a rejected editor save.
  await editor.fill('Must not overwrite external edits');
  await fs.writeFile(path.join(project, 'hello.txt'), 'External writer');
  await editor.press('Control+s');
  await page.getByText(/File changed on disk/).waitFor();
  assert.equal(await fs.readFile(path.join(project, 'hello.txt'), 'utf8'), 'External writer');
  await page.getByRole('button', { name: 'Reload', exact: true }).click();
  await page.getByRole('button', { name: 'Discard', exact: true }).click();
  await page.waitForFunction(
    () => document.querySelector('[aria-label="File content"]')?.value === 'External writer',
  );

  await page.getByRole('button', { name: 'New file', exact: true }).click();
  await page.getByRole('textbox', { name: 'Filename' }).fill('new.txt');
  await page.getByRole('button', { name: 'Confirm', exact: true }).click();
  await page.getByRole('button', { name: 'new.txt', exact: true }).click();
  await page.getByRole('button', { name: 'Rename / move', exact: true }).click();
  await page.getByRole('textbox', { name: 'Filename' }).fill('renamed.txt');
  await page.getByRole('button', { name: 'Confirm', exact: true }).click();
  await page.getByRole('button', { name: 'renamed.txt', exact: true }).click();
  await fs.access(path.join(project, 'renamed.txt'));
  await page.getByRole('button', { name: 'Delete…', exact: true }).click();
  await page.getByRole('button', { name: 'Delete permanently', exact: true }).click();
  await page.getByRole('button', { name: 'renamed.txt', exact: true }).waitFor({ state: 'hidden' });
  await assert.rejects(fs.access(path.join(project, 'renamed.txt')));

  await page.getByRole('button', { name: 'New folder', exact: true }).click();
  await page.getByRole('textbox', { name: 'Filename' }).fill('новая папка');
  await page.getByRole('button', { name: 'Confirm', exact: true }).click();
  await page.getByRole('button', { name: 'новая папка', exact: true }).click();
  await page.getByRole('button', { name: 'Parent folder', exact: true }).click();
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
  await page.getByRole('button', { name: 'Close dialog', exact: true }).click();
  await page.getByText(/Discard unsaved changes/).waitFor();
  await page.getByRole('button', { name: 'Keep editing', exact: true }).click();
  assert.equal(await editor.inputValue(), 'Unsaved changes');
  const windowCloser = spawn(
    'powershell.exe',
    ['-NoProfile', '-Command', `(Get-Process -Id ${child.pid}).CloseMainWindow() | Out-Null`],
    { windowsHide: true, stdio: 'ignore' },
  );
  await new Promise((resolve) => windowCloser.once('exit', resolve));
  await page.getByText(/Discard unsaved changes/).waitFor();
  assert(!exited, 'Native window closed despite unsaved edits');
  await page.getByRole('button', { name: 'Keep editing', exact: true }).click();
  // Global shortcuts must not replace the editor and silently discard its buffer.
  await editor.press('Control+k');
  assert.equal(await editor.inputValue(), 'Unsaved changes');
  await page.getByRole('button', { name: 'Close dialog', exact: true }).click();
  await page.getByRole('button', { name: 'Discard', exact: true }).click();
  await page.getByRole('button', { name: 'Project files', exact: true }).click();
  await page.getByRole('button', { name: 'hello.txt', exact: true }).click();
  await fs.mkdir('test-results', { recursive: true });
  await page.screenshot({ path: 'test-results/native-files.png' });
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
