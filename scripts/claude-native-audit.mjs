// Test the packaged GUI executable's documented exec-form hook entry, without a model request.
import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { randomUUID } from 'node:crypto';
import net from 'node:net';
import path from 'node:path';

if (process.platform !== 'win32') throw new Error('Claude native hook audit requires Windows');
const binary = path.resolve('target/release/bebekoncode-desktop.exe');
const pipe = `\\\\.\\pipe\\bebekon-claude-${randomUUID()}`;
const request = {
  hook_event_name: 'PermissionRequest',
  tool_name: 'Write',
  tool_input: { file_path: 'проект с пробелами\\test.txt', content: 'test only' },
};
const decision = {
  hookSpecificOutput: {
    hookEventName: 'PermissionRequest',
    decision: { behavior: 'deny', message: 'Test user denied' },
  },
};
const server = net.createServer((socket) => {
  let input = '';
  socket.on('data', (chunk) => {
    input += chunk;
    if (input.includes('\n')) {
      assert.deepEqual(JSON.parse(input), request);
      socket.end(JSON.stringify(decision) + '\n');
    }
  });
});
await new Promise((resolve, reject) => {
  server.once('error', reject);
  server.listen(pipe, resolve);
});
async function invoke(target, value) {
  const child = spawn(binary, ['--claude-hook', target], {
    windowsHide: true,
    stdio: ['pipe', 'pipe', 'pipe'],
  });
  let output = '';
  child.stdout.on('data', (chunk) => {
    output += chunk;
  });
  child.stdin.end(JSON.stringify(value));
  const timer = setTimeout(() => child.kill(), 10_000);
  try {
    const exit = await new Promise((resolve, reject) => {
      child.once('error', reject);
      child.once('exit', resolve);
    });
    assert.equal(exit, 0);
    return JSON.parse(output);
  } finally {
    clearTimeout(timer);
  }
}
try {
  assert.deepEqual(await invoke(pipe, request), decision);
  const missing = await invoke(pipe + '-missing', request);
  assert.equal(missing.hookSpecificOutput.decision.behavior, 'deny');
  const invalid = await invoke('not-a-local-bridge', request);
  assert.equal(invalid.hookSpecificOutput.permissionDecision, 'deny');
  console.log(
    'PASS: packaged EXE hooks preserve Unicode, return the user decision, and deny unavailable/invalid bridges',
  );
} finally {
  await new Promise((resolve) => server.close(resolve));
}
