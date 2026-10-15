// Packaged startup against LF release and CRLF local-build databases. No model calls.
import assert from 'node:assert/strict';
import { mkdtemp, readFile, readdir } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { createHash } from 'node:crypto';
import { DatabaseSync } from 'node:sqlite';
import { spawn } from 'node:child_process';
import { chromium } from '@playwright/test';

const binary = resolve(process.env.BEBEKON_TEST_BINARY || 'target/release/bebekoncode-desktop.exe');
const migrations = new URL('../crates/agent-core/migrations/', import.meta.url);
for (const legacy of [true, false]) {
  const root = await mkdtemp(join(tmpdir(), 'bebekon-upgrade-audit-'));
  const database = join(root, 'workspace.db');
  const db = new DatabaseSync(database);
  db.exec(`CREATE TABLE _sqlx_migrations (version BIGINT PRIMARY KEY, description TEXT NOT NULL,
    installed_on TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP, success BOOLEAN NOT NULL,
    checksum BLOB NOT NULL, execution_time BIGINT NOT NULL)`);
  for (const file of (await readdir(migrations)).filter((f) => f.endsWith('.sql')).sort()) {
    const version = Number(file.split('_')[0]);
    const sql = (await readFile(new URL(file, migrations), 'utf8')).replaceAll('\r\n', '\n');
    db.exec(sql);
    const historical = legacy && [6, 7].includes(version) ? sql.replaceAll('\n', '\r\n') : sql;
    db.prepare(
      'INSERT INTO _sqlx_migrations (version,description,success,checksum,execution_time) VALUES (?,?,1,?,0)',
    ).run(version, file, createHash('sha384').update(historical).digest());
  }
  db.prepare('INSERT INTO workspaces VALUES (?,?,?,0)').run('project', 'Upgrade fixture', root);
  db.prepare(
    `INSERT INTO sessions (id,workspace_id,provider,account_profile_id,model,title,status,
    permission_profile,working_directory,created_at,updated_at) VALUES ('saved-chat','project',
    'mock','mock-local','mock-stream-v1','Чат до обновления','idle','workspace_auto',?,0,0)`,
  ).run(root);
  db.prepare('INSERT INTO events (session_id,run_id,timestamp,payload) VALUES (?,?,0,?)').run(
    'saved-chat',
    'saved-run',
    JSON.stringify({ type: 'assistant_text_delta', text: 'Сохранённая история' }),
  );
  const before = db
    .prepare('SELECT version,hex(checksum) AS checksum FROM _sqlx_migrations ORDER BY version')
    .all();
  db.close();
  const child = spawn(binary, ['--data-dir', root], {
    windowsHide: true,
    stdio: 'ignore',
    env: {
      ...process.env,
      WEBVIEW2_USER_DATA_FOLDER: join(root, 'webview'),
      WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS:
        '--remote-debugging-port=9241 --remote-debugging-address=127.0.0.1',
    },
  });
  const exited = new Promise((resolveExit) => child.once('exit', resolveExit));
  let browser;
  try {
    for (let i = 0; i < 100; i++) {
      if (child.exitCode !== null)
        throw new Error(`Startup failed for ${legacy ? 'CRLF' : 'LF'} database: ${child.exitCode}`);
      try {
        browser = await chromium.connectOverCDP('http://127.0.0.1:9241');
        break;
      } catch {
        await new Promise((r) => setTimeout(r, 200));
      }
    }
    assert(browser, 'WebView did not start');
    let page;
    for (let i = 0; i < 50; i++) {
      if (child.exitCode !== null)
        throw new Error(`Application exited during setup: ${child.exitCode}`);
      page = browser
        .contexts()
        .flatMap((c) => c.pages())
        .find((p) => p.url().includes('tauri.localhost'));
      if (page) break;
      await new Promise((r) => setTimeout(r, 200));
    }
    assert(page, 'Application page missing');
    await page.waitForFunction(() => !!window.__TAURI_INTERNALS__);
    const result = await page.evaluate(async () => {
      const invoke = window.__TAURI_INTERNALS__.invoke;
      return {
        snapshot: await invoke('snapshot'),
        events: await invoke('session_events', { sessionId: 'saved-chat', before: null }),
      };
    });
    assert.equal(
      result.snapshot.sessions.find((s) => s.id === 'saved-chat')?.title,
      'Чат до обновления',
    );
    assert(result.snapshot.accounts.some((a) => a.id === 'mock-local'));
    assert(result.events.some((e) => e.payload.text === 'Сохранённая история'));
    const reopened = new DatabaseSync(database, { readOnly: true });
    assert.deepEqual(
      reopened
        .prepare('SELECT version,hex(checksum) AS checksum FROM _sqlx_migrations ORDER BY version')
        .all(),
      before,
    );
    reopened.close();
    console.log(
      `PASS: ${legacy ? 'legacy CRLF' : 'release LF'} database opens; chats, accounts, events and migration history preserved; model calls=0`,
    );
  } finally {
    await browser?.close();
    child.kill();
    await exited;
  }
}
