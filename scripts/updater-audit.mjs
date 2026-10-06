// Test-only loopback fixture. The audit executable downloads/verifies but never installs.
import fs from 'node:fs/promises';
import http from 'node:http';
import { spawn } from 'node:child_process';
import path from 'node:path';
import os from 'node:os';

const manifest = JSON.parse(await fs.readFile('target/release/latest.json', 'utf8'));
const target = manifest.platforms['windows-x86_64'];
const filename = decodeURIComponent(new URL(target.url).pathname.split('/').at(-1));
const bytes = await fs.readFile(path.join('target/release/bundle/nsis', filename));
const altered = Buffer.from(bytes);
altered[altered.length - 1] ^= 1;
const server = http.createServer((request, response) => {
  if (request.url === '/latest.json') {
    const fixture = structuredClone(manifest);
    fixture.platforms['windows-x86_64'].url = `http://127.0.0.1:${server.address().port}/setup.exe`;
    response.writeHead(200, { 'Content-Type': 'application/json' });
    response.end(JSON.stringify(fixture));
  } else if (request.url === '/setup.exe' || request.url === '/tampered.exe') {
    response.writeHead(200, {
      'Content-Type': 'application/octet-stream',
      'Content-Length': bytes.length,
    });
    response.end(request.url === '/setup.exe' ? bytes : altered);
  } else {
    response.writeHead(404);
    response.end();
  }
});
await new Promise((resolve) => server.listen(0, '127.0.0.1', resolve));
try {
  // Cargo examples do not receive the main application's Windows resource manifest.
  // Common Controls v6 is required by Tauri's TaskDialogIndirect import.
  await fs.mkdir('target/release/examples', { recursive: true });
  await fs.writeFile(
    'target/release/examples/updater_audit.exe.manifest',
    '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>' +
      '<assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0"><dependency><dependentAssembly>' +
      '<assemblyIdentity type="win32" name="Microsoft.Windows.Common-Controls" version="6.0.0.0" processorArchitecture="*" publicKeyToken="6595b64144ccf1df" language="*" />' +
      '</dependentAssembly></dependency></assembly>',
  );
  const cargo = path.join(os.homedir(), '.cargo/bin/cargo.exe');
  const child = spawn(
    cargo,
    [
      'run',
      '-p',
      'bebekoncode-desktop',
      '--release',
      '--example',
      'updater_audit',
      '--locked',
      '--offline',
      '--',
      `http://127.0.0.1:${server.address().port}/latest.json`,
    ],
    { stdio: 'inherit', windowsHide: true },
  );
  process.exitCode = await new Promise((resolve, reject) => {
    child.once('exit', (code) => resolve(code ?? 1));
    child.once('error', reject);
  });
} finally {
  server.close();
}
