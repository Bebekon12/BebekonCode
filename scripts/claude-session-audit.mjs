// Offline contract check for the pinned official SDK's local fork operation.
import assert from 'node:assert/strict';
import { mkdtemp, mkdir, writeFile, readFile, readdir } from 'node:fs/promises';
import { join } from 'node:path';
import { tmpdir } from 'node:os';
import { randomUUID } from 'node:crypto';
import { execFileSync } from 'node:child_process';
const root = await mkdtemp(join(tmpdir(), 'bebekon-sdk-fork-'));
const project = join(root, 'project');
const profile = join(root, 'profile');
await mkdir(project);
const directory = join(profile, 'projects', project.replace(/[^a-zA-Z0-9]/g, '-'));
await mkdir(directory, { recursive: true });
const sessionId = randomUUID();
const messageId = randomUUID();
const text =
  JSON.stringify({
    type: 'user',
    uuid: messageId,
    parentUuid: null,
    sessionId,
    cwd: project,
    isSidechain: false,
    timestamp: new Date().toISOString(),
    version: '2.1.295',
    message: { role: 'user', content: 'Offline fixture only' },
  }) + '\n';
const original = join(directory, `${sessionId}.jsonl`);
await writeFile(original, text);
const output = execFileSync(process.execPath, ['scripts/claude-session.mjs', sessionId, project], {
  windowsHide: true,
  encoding: 'utf8',
  timeout: 15000,
  env: {
    PATH: process.env.PATH,
    SystemRoot: process.env.SystemRoot,
    TEMP: root,
    TMP: root,
    CLAUDE_CONFIG_DIR: profile,
  },
});
const fork = JSON.parse(output).session_id;
assert.match(fork, /^[0-9a-f-]{36}$/);
assert.notEqual(fork, sessionId);
assert.equal(await readFile(original, 'utf8'), text);
assert((await readdir(directory)).includes(`${fork}.jsonl`));
console.log(
  'PASS: official SDK offline fork creates a separate session, source unchanged; model calls = 0',
);
